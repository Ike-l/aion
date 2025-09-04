use std::{any::TypeId, collections::{HashMap, HashSet}, sync::{atomic::{AtomicUsize, Ordering}, Arc, RwLock, TryLockError}, task::{Context, Poll, Waker}, thread::JoinHandle};

use tracing::{event, Level};

use crate::{id::Id, parameters::injections::{owned::Owned, shared::Shared, unique::Unique}, scheduler::{accesses::{access::Access, access_map::AccessMap}, blacklists::Blacklists, events::{CurrentEvents, NewEvents}, execution_graph::{ordering::SchedulerOrdering, panic_safe_graphs::PanicSafeGraphs, ExecutionGraph}, interrupts::{CurrentInterrupts, NewInterrupts}, phase::Phase, resources::{new_resources::NewResources, resource_map::{access_checked_resource_map::AccessCheckedResourceMap, ResourceMap}, system_resource::{system_resource_ptr::SystemResourcePtr, SystemResource}, Resource}, system_event::{SystemEvent, SystemResult}, tick::{current_tick::{tick_incrementor, CurrentTick}, lifetime::Lifetime, Tick}}, systems::{async_system::waker::DummyWaker, stored_system::{inner_stored_system::InnerStoredSystem, Criteria, StoredSystem}, system_cell::SystemCell, system_flag::SystemFlag, system_status::SystemStatus}};

pub mod accesses;
pub mod resources;
pub mod execution_graph;
pub mod blacklists;
pub mod tick;
pub mod system_event;

pub mod interrupts;
pub mod events;
pub mod phase;
pub mod builder;


#[derive(Debug)]
pub struct Scheduler {
    // Notes: Always check if resource is being used in background_accesses
    resources: Arc<parking_lot::RwLock<ResourceMap>>,
    system_resources: Arc<HashMap<Id, Arc<SystemResource>>>,

    ids: Arc<RwLock<HashMap<u64, String>>>,
    generations: HashMap<u64, u64>,

    systems: Option<HashMap<Id, StoredSystem>>,

    bubbles: Vec<(Lifetime, String, fn(&HashSet<Id>) -> bool)>,
    bubble_echoes: Vec<(Lifetime, (Lifetime, String, fn(&HashSet<Id>) -> bool))>,
    catfishes: Vec<(Id, Id)>,

    default_systems: Vec<Id>,
    
    current_background_systems: Vec<(Id, JoinHandle<InnerStoredSystem>)>,

    background_accesses: Arc<tokio::sync::RwLock<HashMap<Id, AccessMap>>>,

    threadpool: threadpool::ThreadPool
}

// Default needed to uphold safety
impl Default for Scheduler {
    fn default() -> Self {
        let mut scheduler = Self::new_empty(8);

        let tick_id = scheduler.insert_system(
            "Standard Tick Accumulator".to_string(),
            InnerStoredSystem::new_sync(tick_incrementor),
            |events| { 
                events.contains(&Id::from(&Phase::PreProcessing))
            },
            SchedulerOrdering::default(),
            HashSet::new(),
            // None since my criteria fn already does it
            None
        );

        scheduler.default_systems.push(tick_id);
        
        {
            let mut resources = scheduler.resources.write();
            unsafe { resources.insert_auto_default::<Blacklists>() };
            unsafe { resources.insert_auto_default::<CurrentTick>() };
            unsafe { resources.insert_auto_default::<NewResources>() };
            unsafe { resources.insert_auto_default::<NewEvents>() };
            unsafe { resources.insert_auto_default::<CurrentEvents>() };
            unsafe { resources.insert_auto_default::<NewInterrupts>() };
            unsafe { resources.insert_auto_default::<CurrentInterrupts>() };
        }
        {
            let resources = scheduler.resources.read();
            let mut blacklists = unsafe { resources.resolve::<Unique<Blacklists>>().unwrap() };

            for blacklist in blacklists.blacklists.values_mut() {
                blacklist.insert_typed_blacklist_auto::<NewResources>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<CurrentEvents>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<CurrentInterrupts>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<Blacklists>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let background = blacklists.get_mut(&Phase::BackgroundStart).unwrap();
                background.insert_access_blacklist(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                background.insert_typed_blacklist_auto::<Blacklists>(Access::Shared, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let executing = blacklists.get_mut(&Phase::Processing).unwrap();
                executing.insert_typed_blacklist_auto::<CurrentTick>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let finishing = blacklists.get_mut(&Phase::PostProcessing).unwrap();
                finishing.insert_typed_blacklist_auto::<CurrentTick>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }
        }

        scheduler
    }
}

impl Scheduler {    
    fn new_empty(threads: usize) -> Self {
        assert_ne!(threads, 0, "Scheduler must have 1+ threads");
        
        Self {
            resources: Arc::new(parking_lot::RwLock::new(ResourceMap::default())),
            system_resources: Arc::new(HashMap::new()),
            ids: Arc::new(RwLock::new(HashMap::new())),
            generations: HashMap::new(),
            systems: Some(HashMap::new()),
            bubbles: Vec::new(),
            bubble_echoes: Vec::new(),
            catfishes: Vec::new(),
            default_systems: Vec::new(),
            current_background_systems: Vec::new(),
            background_accesses: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            threadpool: threadpool::Builder::new().thread_name("Scheduler's Execution Thread".to_string()).num_threads(threads).build()
        }
    }

    pub fn check_orderering(&self) -> Option<(&StoredSystem, &Id)> {
        for (_, system) in self.systems.as_ref().unwrap() {
            let ids = self.ids.read().unwrap();

            let satisfied = system.ordering().after.iter().find(|dependent| !ids.contains_key(dependent.id()));
            if let Some(id) = satisfied {
                return Some((system, id));
            }

            let satisfied = system.ordering().before.iter().find(|dependent| !ids.contains_key(dependent.id()));
            if let Some(id) = satisfied {
                return Some((system, id));
            }
        }

        None
    }

    pub fn current_tick(&self) -> Tick {
        // Safety:
        // Is `copy` so doesnt `access` per se
        unsafe { self.resources.read().resolve::<Shared<CurrentTick>>().unwrap().tick }
    }

    pub fn insert_system<F>(
        &mut self,
        display_name: String,
        system: InnerStoredSystem,
        wake_up_criteria: F,
        ordering: SchedulerOrdering,
        mut flags: HashSet<SystemFlag>,  
        phase: Option<Phase>,      
    ) -> Id 
        where F: Fn(&HashSet<Id>) -> bool + Send + Sync + 'static
    {
        let system_id = Id::new(display_name.as_str(), &mut self.generations);
        self.ids.write().unwrap().insert(system_id.id().clone(), display_name.clone());

        let wake_up_criteria: Box<dyn Fn(&HashSet<Id>) -> bool + Send + Sync> = if let Some(phase) = phase {
            Box::new(move |events| wake_up_criteria(events) && events.contains(&Id::from(&phase)))
        } else {
            Box::new(wake_up_criteria)
        };

        #[cfg(any(test, debug_assertions))]
        flags.extend([SystemFlag::HasRequirements, SystemFlag::NotBlacklisted, SystemFlag::Succeeds]);

        let system = StoredSystem::new(system, Criteria(wake_up_criteria), display_name, ordering, flags);

        self.systems.as_mut().unwrap().insert(system_id.clone(), system);

        Arc::get_mut(&mut self.system_resources).unwrap().insert(system_id.clone(), Arc::new(SystemResource::default()));

        system_id
    }

    /// Acts like a system that doesnt run
    pub fn insert_bubble(
        &mut self,
        display_name: String,
        wake_up_criteria: fn(&HashSet<Id>) -> bool
    ) {
        self.insert_bubble_column(Lifetime::new(self.current_tick(), Tick(1)), display_name, wake_up_criteria);
    }

    /// Bubble exists for the lifetime
    pub fn insert_bubble_column(
        &mut self, 
        lifetime: Lifetime, 
        display_name: String, 
        wake_up_criteria: fn(&HashSet<Id>) -> bool
    ) {
        self.bubbles.push((lifetime, display_name, wake_up_criteria));
    }

    /// Bubble waits the lifetime 
    pub fn insert_bubble_echo(
        &mut self,
        echo_lifetime: Lifetime,
        bubble_lifetime: Lifetime,
        display_name: String,
        wake_up_criteria: fn(&HashSet<Id>) -> bool
    ) {
        self.bubble_echoes.push((echo_lifetime, (bubble_lifetime, display_name, wake_up_criteria)))
    }

    /// catfish an event: when the event is noticed, create another specified event
    pub fn catfish(
        &mut self,
        target_event: Id,
        to_insert: Id,
    ) {
        self.catfishes.push((target_event, to_insert));
    }

    pub fn resources(&self) -> AccessCheckedResourceMap<'_> {
        AccessCheckedResourceMap::new(&self.resources, &self.background_accesses)
    }

    pub fn resolve_owned<T, S>(&self) -> Option<Owned<T, S>> 
        where S: ToOwned<Owned = T> + 'static,
    {
        // Safety:
        // doesnt return a reference
        unsafe { self.resources.read().resolve::<Owned<T, S>>() }
    }

    pub fn insert_new_event<T: Into<Id>>(&mut self, event: T) {
        // Safety:
        // Uses locks internally so multiple access is fine
        unsafe { self.resources.read().resolve::<Unique<NewEvents>>().unwrap().insert(event.into()) };
    }

    pub fn remove_new_event<T: Into<Id>>(&mut self, event: T) {
        // Safety:
        // Uses locks internally so multiple access is fine
        unsafe { self.resources.read().resolve::<Unique<NewEvents>>().unwrap().remove(event.into()) };
    }

    /// Safety:
    /// Ensure no reference alive when insert
    pub unsafe fn insert<T: 'static>(&mut self, type_id: TypeId, resource: T) -> Option<Resource> {
        unsafe { self.resources.write().insert(type_id, resource) }
    }

    /// Safety:
    /// Ensure no reference alive when insert
    pub unsafe fn insert_auto<T: 'static>(&mut self, resource: T) -> Option<Resource> {
        unsafe { self.insert(TypeId::of::<T>(), resource) }
    }

    /// Safety:
    /// Ensure no reference alive when insert
    pub unsafe fn insert_auto_default<T: 'static + Default>(&mut self) -> Option<Resource> {
        unsafe { self.insert_auto(T::default()) }
    }

    pub fn conservatively_insert<T: 'static>(&mut self, type_id: TypeId, resource: T) -> anyhow::Result<()> {
        self.resources.read().conservatively_insert(type_id, resource)
    }

    pub fn conservatively_insert_auto<T: 'static>(&mut self, resource: T) -> anyhow::Result<()> {
        self.conservatively_insert(TypeId::of::<T>(), resource)
    }

    pub fn conservatively_insert_auto_default<T: 'static + Default>(&mut self) -> anyhow::Result<()> {
        self.conservatively_insert_auto(T::default())
    }

    pub fn tick(&mut self) {
        let mut phases = Phase::iter_fields();
        // Safety:
        // Uses locks internally
        {
            let phase = phases.next().unwrap();
            assert_eq!(phase, Phase::Ticking);

            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {:?}", phase);
            let resource_guard = self.resources.read();


            // Safety:
            // Uses locks internally
            let mut current_events = unsafe { resource_guard.resolve::<Unique<CurrentEvents>>().unwrap() };
            current_events.tick(
                // Safety:
                // Uses locks internally
                *unsafe { resource_guard.resolve::<Unique<NewEvents>>().unwrap() }
            );

            for (target, insert) in self.catfishes.iter() {
                if current_events.events().contains(&target) {
                    current_events.insert(insert.clone());
                }
            }

            // Safety:
            // Uses locks internally
            let mut current_interrupts = unsafe { resource_guard.resolve::<Unique<CurrentInterrupts>>().unwrap() };
            current_interrupts.tick(
                // Safety:
                // Uses locks internally
                *unsafe { resource_guard.resolve::<Unique<NewInterrupts>>().unwrap() }
            );
            current_interrupts.extend(self.current_background_systems.iter().map(|(system_id, _)| system_id.clone()));

            // Safety:
            // Blacklists blacklisted from background processes
            unsafe { resource_guard.resolve::<Unique<Blacklists>>().unwrap().tick() }; 
        }

        // skip `Ticking`, take `PreProcessing, Processing, PostProcessing`
        for _ in 0..3 {
            let phase = phases.next().unwrap();
            println!("Executing phase: {phase:?}");
            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {phase:?}");

            let (execution_graphs, running_systems, bubbles) = {
                let resource_guard = self.resources.read();

                //self.bubbles.retain_mut(|(lifetime, _, _)| lifetime.tick());

                if !self.bubble_echoes.is_empty() {
                    todo!()
                }
                // TODO bubble echoes -> only start ticking if criteria satisfied
                // then when ticked create a bubble, restart timer & stop ticking
                // for (mut lifetime, bubble) in self.bubble_echoes.drain(..).collect::<Vec<_>>() {
                //     if lifetime.tick() {
                //         self.bubble_echoes.push((lifetime, bubble));
                //     } else {
                //         self.bubbles.push(bubble);
                //     }
                // }

                // Safety:
                // Uses locks internally
                let mut current_events = unsafe { resource_guard.resolve::<Unique<CurrentEvents>>().unwrap() };
                current_events.insert(&phase);

    
                // Safety:
                // no other system can run with mutable access (and uses locks)
                let current_interrupts = unsafe { resource_guard.resolve::<Shared<CurrentInterrupts>>().unwrap() };    
                
    
                // Safety:
                // Background threads only have read access and no other systems are runnign
                let blacklist = unsafe { resource_guard.resolve::<Shared<Blacklists>>().unwrap().get(&phase).unwrap() };
                let resource_keys = resource_guard.keys().collect();
                let system_ptrs = self.systems.as_ref().unwrap().iter().filter_map(|(system_id, system)| {
                    if system.flags().contains(&SystemFlag::Blocking) || !(system.flags().contains(&SystemFlag::NonBlocking) || system.flags().contains(&SystemFlag::Blocking)) {
                        Some((system_id.clone(), system))
                    } else {
                        None
                    }
                });
    
                let to_execute = system_ptrs
                    .filter(|(s, _)| !current_interrupts.contains(s))
                    .filter(|(_, s)| s.wake_up(&current_events.events()))
                    .filter(|(_, s)| {
                        if s.test_criteria(&resource_keys) {
                            true
                        } else {
                            if s.flags().contains(&SystemFlag::HasRequirements) {
                                panic!("System requirements have not been met for: {}", s.display_name())
                            } else {
                                false
                            }
                        }
                    })
                    .filter(|(_, s)| {
                        if !blacklist.check_blocked(s.cached_accesses().scheduler()) {
                            true
                        } else {
                            if s.flags().contains(&SystemFlag::NotBlacklisted) {
                                panic!("System: {}, has been blocked by a blacklist", s.display_name())
                            } else {
                                false
                            }
                        }
                    })
                    .collect::<Vec<_>>();
    
                let running_systems = to_execute.iter().map(|(system_id, _)| system_id.clone()).collect::<HashSet<_>>();

                let bubbles = self.bubbles.iter().filter_map(|(_, bubble_name, crit)| {
                    if crit(&current_events.events()) {
                        Some(Id::from(bubble_name.as_str()))
                    } else {
                        None
                    }
                }).collect::<Vec<_>>();

                (if to_execute.is_empty() {
                    vec![]
                } else if to_execute.len() > self.threadpool.max_count() {
                    Self::lift_independent(to_execute.into_iter())
                        .map(|systems| {
                            systems.into_iter().map(|(stored_system, system_id)| {
                                (system_id, stored_system.ordering())
                            }).collect::<Vec<_>>()
                        })
                        .map(|systems| tokio::sync::RwLock::new(ExecutionGraph::new(&systems)))
                        .collect()
                } else {
                    let systems = to_execute.into_iter().map(|(system_id, stored_system)| {
                        (system_id, stored_system.ordering())  
                    }).collect::<Vec<_>>();

                    vec![
                        tokio::sync::RwLock::new(ExecutionGraph::new(&systems))
                    ]
                }, running_systems, bubbles)
            };

            if !execution_graphs.is_empty() {
                for system_id in running_systems {
                    //println!("System Id: {system_id:?}");
                    self.insert_new_event::<Id>(system_id.into())
                }

                for bubble in bubbles {
                    self.insert_new_event(bubble);
                }
                // for system in bubbles that are triggered create new event

                let (systems, errors) = Self::execute_graphs(
                    self.systems.take().unwrap(),
                    &self.resources,
                    &self.system_resources,
                    &self.ids,
                    PanicSafeGraphs::new(Arc::new(execution_graphs)),
                    &self.background_accesses,
                    &self.threadpool
                );

                for (system_id, system_result) in errors {
                    if systems.get(&system_id).unwrap().flags().contains(&SystemFlag::Succeeds) {
                        panic!("{:?}", system_result);
                    }

                    event!(Level::WARN, system_id = ?system_id, system_result = ?system_result);

                    match system_result {
                        SystemResult::Success => {},
                        SystemResult::Error(error) => {
                            panic!("{}", error)
                        }
                        SystemResult::SystemEvent(event) => {
                            match event {
                                SystemEvent::NoSystemEvent => self.remove_new_event::<Id>(system_id.clone().into()),
                                // SystemEvent::FailSystemEvent => {
                                //     let new_event = {
                                //         let ids = self.ids.read().unwrap();
                                //         Id::from(format!("{}Failed", ids.get(&system_id).unwrap()).as_str())
                                //     };
                
                                //     self.insert_new_event(new_event);
                                // }
                                SystemEvent::SignalEvent(signal) => {
                                    let new_event = {
                                        let ids = self.ids.read().unwrap();
                                        Id::from(format!("{}{signal}", ids.get(system_id.id()).unwrap()).as_str())
                                    };
                
                                    self.insert_new_event(new_event);
                                }
                            }
                        }
                    }
                }

                let _ = self.systems.insert(systems);
            }

            let resource_guard = self.resources.read();
            // Safety:
            // Uses locks internally
            let mut current_events = unsafe { resource_guard.resolve::<Unique<CurrentEvents>>().unwrap() };
            current_events.remove(&Id::from(&phase));
        }

        {
            let phase = phases.next().unwrap();
            assert_eq!(phase, Phase::BackgroundEnd);

            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {:?}", phase);
            let mut retain = Vec::new();
            for (system_id, join_handle) in self.current_background_systems.drain(..).collect::<Vec<_>>() {
                if join_handle.is_finished() {
                    println!("System Finished: {:?}", self.ids.read().unwrap().get(system_id.id()));

                    let system = self.systems.as_mut().unwrap().get_mut(&system_id).unwrap();
    
                    let inner_system = join_handle.join().unwrap();
                    system.insert_system(inner_system);
    
                    self.background_accesses.blocking_write().remove(&system_id);
    
                    *system.status().lock().unwrap() = SystemStatus::Init;
    
                    self.insert_new_event::<Id>(system_id.into());

                } else {
                    retain.push((system_id, join_handle));
                }
            }

            self.current_background_systems.extend(retain);
        }

        {
            let phase = phases.next().unwrap();
            assert_eq!(phase, Phase::BackgroundStart);

            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {:?}", phase);
            let to_execute = {
                let resource_guard = self.resources.read();
                
                // Safety:
                // Uses locks internally
                let mut current_events = unsafe { resource_guard.resolve::<Unique<CurrentEvents>>().unwrap() };
                current_events.insert(&phase);
                    
                // Safety:
                // Uses locks internally
                let mut current_interrupts = unsafe { resource_guard.resolve::<Unique<CurrentInterrupts>>().unwrap() };
                current_interrupts.extend(self.current_background_systems.iter().map(|(system_id, _)| system_id.clone()));
                        
                // Safety:
                // Blacklists blacklisted from background processes
                let blacklist = unsafe { resource_guard.resolve::<Shared<Blacklists>>().unwrap().get(&phase).unwrap() };
                let resource_keys = resource_guard.keys().collect();
                let system_ptrs = self.systems.as_ref().unwrap().iter().filter_map(|(system_id, system)| {
                    if system.flags().contains(&SystemFlag::NonBlocking) {
                        Some((system_id.clone(), system))
                    } else {
                        None
                    }
                });
        
                system_ptrs
                    .filter(|(_, s)| s.system().is_some())
                    .filter(|(s, _)| !current_interrupts.contains(s))
                    .filter(|(_, s)| s.wake_up(&current_events.events()))
                    .filter(|(_, s)| {
                        if s.test_criteria(&resource_keys) {
                            true
                        } else {
                            if s.flags().contains(&SystemFlag::HasRequirements) {
                                panic!("System requirements have not been met for {}", s.display_name())
                            } else {
                                false
                            }
                        }
                    })
                    .filter(|(_, s)| {
                        if !blacklist.check_blocked(s.cached_accesses().scheduler()) {
                            true
                        } else {
                            if s.flags().contains(&SystemFlag::NotBlacklisted) {
                                panic!("System: {}, has been blocked by a blacklist", s.display_name())
                            } else {
                                false
                            }
                        }
                    }).map(|(system_id, _)| system_id)
                    .collect::<HashSet<_>>()
            };

            let mut systems = self.systems.take().unwrap();
            let mut inner_stored_systems = systems.iter_mut().filter_map(|(system_id, system)| {
                if to_execute.contains(system_id) {
                    Some((system_id.clone(), SystemCell::new(system.take_system().unwrap())))
                } else {
                    None
                }
            }).collect::<HashMap<_, _>>();

            'systems_walk: for system_id in to_execute {
                let system = systems.get(&system_id).unwrap();
                match system.status().try_lock() {
                    Ok(mut status) => {
                        match *status {
                            SystemStatus::Init => {
                                {
                                    let mut accesses_guard = self.background_accesses.blocking_write();
                                    if accesses_guard.values().any(|access| {
                                        system.cached_accesses().conflicts_scheduler(access)
                                    }) {
                                        continue 'systems_walk;
                                    }

                                    assert!(accesses_guard.insert(system_id.clone(), system.cached_accesses().scheduler().clone()).is_none());
                                }

                                *status = SystemStatus::Executing;

                                let mut inner = inner_stored_systems.remove(&system_id).unwrap().consume();

                                match inner {
                                    InnerStoredSystem::Async(_) => {
                                        unimplemented!()
                                    },
                                    InnerStoredSystem::Sync(_) => {
                                        let scheduler_resource_map = Arc::clone(&self.resources);
                                        let ids = Arc::clone(&self.ids);
                                        let system_resource_maps = Arc::clone(&self.system_resources);

                                        self.current_background_systems.push((
                                            system_id.clone(),
                                            std::thread::spawn(move || {
                                                let scheduler_resource_map = scheduler_resource_map.read_arc();
                                                let system_resource_map = SystemResourcePtr::new(Arc::clone(system_resource_maps.get(&system_id).unwrap())).unwrap();
                                                drop(system_resource_maps);
                                                if let InnerStoredSystem::Sync(sys) = &mut inner {
                                                    // println!("System Running: {:?}", ids.read().unwrap().get(&system_id));
                                                    unsafe { sys.run(
                                                        &scheduler_resource_map, 
                                                        Some(&system_resource_map),
                                                        system_id, 
                                                        ids, 
                                                        None
                                                    ).unwrap() };
                                                }
                                                inner
                                            }),
                                        ));
                                    }
                                }
                            },
                            SystemStatus::Executed | SystemStatus::Executing | SystemStatus::Pending => {
                                unreachable!()
                            }
                        }
                    }
                    Err(err) => {
                        assert!(matches!(err, TryLockError::WouldBlock), "How poison?")
                    }
                }
            }

            for (system_id, system_cell) in inner_stored_systems {
                systems.get_mut(&system_id).unwrap().insert_system(system_cell.consume());
            }

            let _ = self.systems.insert(systems);
        }
    
        {
            let phase = phases.next().unwrap();
            assert_eq!(phase, Phase::Movement);

            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {:?}", phase);
            let resources = self.resources.read();
            // Safety:
            // Uses locks internally
            let new_resources = unsafe { resources.resolve::<Unique<NewResources>>().unwrap().write().drain().collect::<HashMap<_, _>>() };
            for (system_id, resource_map) in new_resources {
                if let Some(system_id) = system_id {
                    let _ = self.system_resources.get(&system_id).unwrap().conservatively_merge(resource_map);
                } else {
                    let _ = resources.conservatively_merge(resource_map);
                }
            }
        }
    }

    pub fn lift_independent<'a, T>(systems: T) -> impl Iterator<Item = Vec<(&'a StoredSystem, Id)>> 
        where T: Iterator<Item = (Id, &'a StoredSystem)>
    {
        let mut independent: Vec<HashSet<Id>> = Vec::new();
        let mut system_mapping = HashMap::new();

        for (system_id, system) in systems {
            let mut current_set = HashSet::new();
            
            current_set.insert(system_id.clone());
            current_set.extend(system.ordering().before.clone());
            current_set.extend(system.ordering().after.clone());

            let mut dependent_sets = Vec::new();
            for (i, set) in independent.iter().enumerate() {
                // len > 0
                if set.intersection(&current_set).next().is_some() {
                    dependent_sets.push(i);
                }
            }

            for i in dependent_sets {
                let set = independent.remove(i);
                current_set.extend(set);
            }

            independent.push(current_set);

            system_mapping.insert(system_id, system);
        }

        independent.into_iter().map(move |v| {
            v.into_iter().fold(Vec::new(), |mut acc, cur| {
                acc.push((system_mapping.remove(&cur).unwrap(), cur));
                acc
            })
        })
    }

    pub fn execute_graphs(
        mut systems: HashMap<Id, StoredSystem>,
        scheduler_resource_map: &Arc<parking_lot::RwLock<ResourceMap>>,
        system_resource_maps: &Arc<HashMap<Id, Arc<SystemResource>>>,
        ids: &Arc<RwLock<HashMap<u64, String>>>,
        execution_graphs: PanicSafeGraphs<Id>,
        accesses: &Arc<tokio::sync::RwLock<HashMap<Id, AccessMap>>>,
        threadpool: &threadpool::ThreadPool,
    ) -> (HashMap<Id, StoredSystem>, Vec<(Id, SystemResult)>) {
        let errors = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let graph_count = execution_graphs.graphs.len();
        // Each graph is responsible for decrementing this. When 0 all are finished
        let finished = Arc::new(AtomicUsize::new(graph_count));

        let system_resource_map_ptrs = Arc::new(system_resource_maps.iter().filter_map(|(system_id, resource_map)| {
            if *systems.get(system_id).unwrap().needs_system_resource() {
                // can't panic since background systems can't take external system resources
                Some((system_id.clone(), SystemResourcePtr::new(Arc::clone(&resource_map)).unwrap()))
            } else {
                None
            }
        }).collect::<HashMap<_, _>>());

        let inner_stored_systems: Arc<HashMap<_, _>> = Arc::new(systems.iter_mut().filter_map(|(system_id, system)| {
            if let Some(system) = system.take_system() {
                Some((system_id.clone(), SystemCell::new(system)))
            } else {
                None
            }
        }).collect());


        let hollow_systems = Arc::new(systems);

        for i in 0..threadpool.max_count() {
            // trial-and-errored this formula in rust playground
            let start_graph = (i * graph_count) / threadpool.max_count();

            let finished = Arc::clone(&finished);
            let execution_graphs = execution_graphs.arc_clone();
            let accesses = Arc::clone(&accesses);
            let scheduler_resource_map = Arc::clone(scheduler_resource_map);
            let system_resource_maps = Arc::clone(system_resource_maps);
            let ids = Arc::clone(ids);
            let inner_stored_systems = Arc::clone(&inner_stored_systems);
            let systems = Arc::clone(&hollow_systems);
            let system_resource_map_ptrs = Arc::clone(&system_resource_map_ptrs);
            let errors = Arc::clone(&errors);
            
            threadpool.execute(
                move || {
                    let scheduler_resource_map = scheduler_resource_map.read_arc();
                    let runtime = tokio::runtime::Runtime::new().unwrap();
                    runtime.block_on(async move {
                        let waker = Waker::from(Arc::new(DummyWaker));
                        let mut context = Context::from_waker(&waker);
                        let mut tasks = Vec::new();

                        let mut current_graph_index = start_graph;

                        while finished.load(Ordering::Acquire) > 0 {
                            let current_graph = execution_graphs.graphs.get(current_graph_index).unwrap();

                            // chain is the count of how many systems were "passed"
                            //  reasons: another thread is handling it, there are access conflicts, its pending and not done
                            let mut chain = 0;

                            // not sure if i could store the leaf count or if that could lead to data races
                            'graphs_walk: while !current_graph.read().await.finished().load(Ordering::Acquire) && chain <= ( 2 * current_graph.read().await.leaves().count()) {
                                let leaf_count = current_graph.read().await.leaves().count();
                                if leaf_count > 0 {
                                    let nth_leaf = if let Some((system_id, status)) = current_graph.read().await.leaves().nth(chain % leaf_count) {
                                        if status.load(Ordering::Acquire) == ExecutionGraph::<Id>::PENDING {
                                            None
                                        } else {
                                            Some(system_id.clone())
                                        }
                                    } else {
                                        None
                                    };

                                    if let Some(system_id) = nth_leaf {
                                        let system = systems.get(&system_id).unwrap();

                                        // This lock helps uphold safety guarantees with SystemCell
                                        match system.status().try_lock() {
                                            Ok(mut status) => {
                                                match *status {
                                                    SystemStatus::Init => {
                                                        {
                                                            let mut accesses_guard = accesses.write().await;
                                                            if accesses_guard.values().any(|access| {
                                                                system.cached_accesses().conflicts_scheduler(access)
                                                            }) {
                                                                chain += 1;
                                                                continue 'graphs_walk;
                                                            }

                                                            assert!(accesses_guard.insert(system_id.clone(), system.cached_accesses().scheduler().clone()).is_none());
                                                        }

                                                        *status = SystemStatus::Executing;
                                                        chain = 0;

                                                        // If there is a "safe" way of getting &mut AsyncSystem without RAII let me know pls 🙏
                                                        // Can't have a RAII because need to store the future in a higher scope
                                                        // regular RefCell doesnt work obviously :P
                                                        // SAFETY:
                                                        // It is safe to mutably dereference the `UnsafeCell` here because:
                                                        // - The `system` is locked via the `status`, which guarantees exclusive access
                                                        //   to this particular `StoredSystemKind` instance.
                                                        // - The `Init` branch for a system can only happen once
                                                        // - Therefore, no other thread or part of the code can access this `UnsafeCell`
                                                        //   (either mutably or immutably) while this function is operating.
                                                        let inner: &mut InnerStoredSystem = unsafe {
                                                            &mut *inner_stored_systems.get(&system_id).unwrap().system.get()
                                                        };

                                                        match inner {
                                                            InnerStoredSystem::Sync(system) => {
                                                                println!("System Running: {:?}", ids.read().unwrap().get(system_id.id()));
                                                                let result = unsafe { system.run(
                                                                        &scheduler_resource_map, 
                                                                        system_resource_map_ptrs.get(&system_id), 
                                                                        system_id.clone(), 
                                                                        Arc::clone(&ids), 
                                                                        Some(&system_resource_maps)
                                                                ) };

                                                                if let Some(result) = result {
                                                                    errors.lock().await.push((system_id.clone(), result));
                                                                }


                                                                *status = SystemStatus::Executed;
                                                                current_graph.write().await.mark_as_complete(&system_id);
                                                                accesses.write().await.remove(&system_id);
                                                                println!("System Finished: {:?}", ids.read().unwrap().get(system_id.id()));
                                                            },
                                                            InnerStoredSystem::Async(system) => {
                                                                println!("System Running: {:?}", ids.read().unwrap().get(system_id.id()));
                                                                let mut task = unsafe { system.run(
                                                                        &scheduler_resource_map, 
                                                                        system_resource_map_ptrs.get(&system_id), 
                                                                        system_id.clone(), 
                                                                        Arc::clone(&ids), 
                                                                        Some(&system_resource_maps)
                                                                ) };

                                                                match task.as_mut().poll(&mut context) {
                                                                    Poll::Pending => {
                                                                        current_graph.write().await.mark_as_pending(&system_id);
                                                                        *status = SystemStatus::Pending;

                                                                        tasks.push((
                                                                            current_graph_index,
                                                                            system_id,
                                                                            task
                                                                        ));
                                                                    },
                                                                    Poll::Ready(result) => {
                                                                        if let Some(result) = result {
                                                                            errors.lock().await.push((system_id.clone(), result));
                                                                        }
                                                                        
                                                                        current_graph.write().await.mark_as_complete(&system_id);
                                                                        *status = SystemStatus::Executed;
                                                                        
                                                                        accesses.write().await.remove(&system_id);

                                                                        println!("System Finished: {:?}", ids.read().unwrap().get(system_id.id()));
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        
                                                        assert_ne!(*status, SystemStatus::Executing);
                                                    },
                                                    SystemStatus::Pending => chain += 1,
                                                    SystemStatus::Executed => { /* Somehow possible but is benign :) */ },
                                                    SystemStatus::Executing => { unreachable!("Somehow got a lock while another thread should be holding it (possible if another thread panics)") }
                                                }
                                            }
                                            Err(_err) => {
                                                //assert!(matches!(err, TryLockError::WouldBlock), "How poison?");

                                                chain += 1;
                                                continue 'graphs_walk;
                                            }
                                        }
                                    }
                                } else {
                                    current_graph.write().await.finished().store(true, Ordering::Release);

                                    let _ = finished.fetch_update(Ordering::SeqCst, Ordering::Relaxed, |finished| {
                                        if finished == 0 {
                                            None
                                        } else {
                                            Some(finished - 1)
                                        }
                                    });
                                }

                                let mut not_done = Vec::new();
                                for (graph_number, system_id, mut fut) in tasks.drain(..) {
                                    match fut.as_mut().poll(&mut context) {
                                        Poll::Pending => {
                                            not_done.push((graph_number, system_id, fut));
                                        },
                                        Poll::Ready(result) => {
                                            if let Some(result) = result {
                                                errors.lock().await.push((system_id.clone(), result));
                                            }

                                            let system = systems.get(&system_id).unwrap();

                                            *system.status().lock().unwrap() = SystemStatus::Executed;
                                            execution_graphs.graphs.get(graph_number).unwrap().write().await.mark_as_complete(&system_id);
                                            accesses.write().await.remove(&system_id);

                                            println!("System Finished: {:?}", ids.read().unwrap().get(system_id.id()));
                                        }
                                    }
                                }

                                tasks.extend(not_done);
                            }

                            current_graph_index = ( current_graph_index + 1 ) % graph_count;
                        }
                    });
                }
            );
        }

        while execution_graphs.drop_signal.load(Ordering::SeqCst) < threadpool.max_count() {
            if execution_graphs.panicked_signal.load(Ordering::SeqCst) {
                panic!("A Scheduler thread has panicked. Choosing to panic the main thread");
            }

            std::hint::spin_loop();
        }

        // the above is functionally the same
        // threadpool.join();

        
        let mut systems = Arc::try_unwrap(hollow_systems).unwrap();

        for (system_id, system_cell) in Arc::try_unwrap(inner_stored_systems).unwrap() {
            let system = systems.get_mut(&system_id).unwrap();
            *system.status().lock().unwrap() = SystemStatus::Init;
            system.insert_system(system_cell.consume());
        }


        (systems, Arc::try_unwrap(errors).unwrap().into_inner())
    }
}
