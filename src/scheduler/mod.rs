use std::{collections::{HashMap, HashSet}, sync::{atomic::{AtomicUsize, Ordering}, Arc, RwLock, TryLockError}, task::{Context, Poll, Waker}, thread::JoinHandle};

use tracing::{event, Level};

use crate::{id::{event_id::SchedulerEvent, system_id::SystemId, Id}, parameters::injections::{shared::Shared, unique::Unique}, scheduler::{accesses::{access::Access, access_map::AccessMap}, blacklists::Blacklists, events::{CurrentEvents, NewEvents}, execution_graph::{ordering::SchedulerOrdering, ExecutionGraph}, interrupts::{CurrentInterrupts, NewInterrupts}, phase::Phase, resources::{new_resources::NewResources, resource_map::ResourceMap, system_resource::{system_resource_ptr::SystemResourcePtr, SystemResource}}, tick::{current_tick::{tick_incrementor, CurrentTick}, lifetime::Lifetime, Tick}}, systems::{async_system::waker::DummyWaker, stored_system::{inner_stored_system::InnerStoredSystem, StoredSystem}, system_cell::SystemCell, system_flag::SystemFlag, system_status::SystemStatus}};

pub mod accesses;
pub mod resources;
pub mod execution_graph;
pub mod blacklists;
pub mod tick;

pub mod interrupts;
pub mod events;
pub mod phase;
pub mod builder;


#[derive(Debug)]
pub struct Scheduler {
    // Notes: Always check if resource is being used in background_accesses
    resources: Arc<parking_lot::RwLock<ResourceMap>>,
    system_resources: Arc<HashMap<SystemId, Arc<SystemResource>>>,

    ids: Arc<RwLock<HashSet<String>>>,
    systems: Option<HashMap<SystemId, StoredSystem>>,
    
    current_background_systems: Vec<(SystemId, JoinHandle<InnerStoredSystem>)>,

    background_accesses: Arc<tokio::sync::RwLock<HashMap<SystemId, AccessMap>>>,

    threadpool: threadpool::ThreadPool
}

impl Default for Scheduler {
    fn default() -> Self {
        let mut scheduler = Self::new_empty(8);

        scheduler.insert_system(
            "Standard Tick Accumulator".to_string(),
            InnerStoredSystem::new_sync(tick_incrementor),
            |events| { 
                events.contains(&SchedulerEvent::from(Id::from(&Phase::Startup)))
            },
            SchedulerOrdering::default(),
            HashSet::new()
        );
        
        {
            let mut resources = scheduler.resources.write();
            resources.insert_auto_default::<Blacklists>();
            resources.insert_auto_default::<CurrentTick>();
            resources.insert_auto_default::<NewResources>();
            resources.insert_auto_default::<NewEvents>();
            resources.insert_auto_default::<CurrentEvents>();
            resources.insert_auto_default::<NewInterrupts>();
            resources.insert_auto_default::<CurrentInterrupts>();
        }
        {
            let resources = scheduler.resources.read();
            let mut blacklists = resources.resolve::<Unique<Blacklists>>().unwrap();

            for blacklist in blacklists.blacklists.values_mut() {
                blacklist.insert_typed_blacklist_auto::<NewResources>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<CurrentEvents>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<CurrentInterrupts>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
                blacklist.insert_typed_blacklist_auto::<Blacklists>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let background = blacklists.get_mut(&Phase::BackgroundStart).unwrap();
                background.insert_access_blacklist(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let executing = blacklists.get_mut(&Phase::Executing).unwrap();
                executing.insert_typed_blacklist_auto::<CurrentTick>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }

            {
                let finishing = blacklists.get_mut(&Phase::Finishing).unwrap();
                finishing.insert_typed_blacklist_auto::<CurrentTick>(Access::Unique, Lifetime::new_perpetual(Tick(0)));
            }
        }

        scheduler
    }
}

impl Scheduler {    
    pub fn new_empty(threads: usize) -> Self {
        assert_ne!(threads, 0, "Scheduler must have 1+ threads");
        
        Self {
            resources: Arc::new(parking_lot::RwLock::new(ResourceMap::default())),
            system_resources: Arc::new(HashMap::new()),
            ids: Arc::new(RwLock::new(HashSet::new())),
            systems: Some(HashMap::new()),
            current_background_systems: Vec::new(),
            background_accesses: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            threadpool: threadpool::ThreadPool::new(threads)
        }
    }

    pub fn insert_system(
        &mut self,
        display_name: String,
        system: InnerStoredSystem,
        wake_up_criteria: fn(&HashSet<SchedulerEvent>) -> bool,
        ordering: SchedulerOrdering,
        flags: HashSet<SystemFlag>,        
    ) -> SystemId {
        let system_id = SystemId::from(Id::from(display_name.as_str()));
        self.ids.write().unwrap().insert(display_name.clone());

        let system = StoredSystem::new(system, wake_up_criteria, display_name, ordering, flags);

        self.systems.as_mut().unwrap().insert(system_id.clone(), system);

        system_id
    }

    pub fn insert_new_event<T: Into<SchedulerEvent>>(&mut self, event: T) {
        self.resources.read().resolve::<Unique<NewEvents>>().unwrap().insert(event.into());
    }

    pub fn tick(&mut self) {
        for phase in Phase::iter_fields().take(3) {
            event!(Level::INFO, phase = ?phase, "Executing Scheduler Phase: {phase:?}");

            let (execution_graphs, running_systems) = {
                let resource_guard = self.resources.read();
                
                let mut current_events = resource_guard.resolve::<Unique<CurrentEvents>>().unwrap();
                current_events.tick(
                    *resource_guard.resolve::<Unique<NewEvents>>().unwrap()
                );
    
                current_events.insert(SchedulerEvent::from(Id::from(&phase)));
    
                let mut current_interrupts = resource_guard.resolve::<Unique<CurrentInterrupts>>().unwrap();
                current_interrupts.tick(
                    *resource_guard.resolve::<Unique<NewInterrupts>>().unwrap()
                );
    
                current_interrupts.extend(self.current_background_systems.iter().map(|(system_id, _)| system_id.clone()));
    
                let blacklist = resource_guard.resolve::<Shared<Blacklists>>().unwrap().get(&phase).unwrap();
                let resource_keys = resource_guard.keys().collect();
                let system_ptrs = self.systems.as_ref().unwrap().iter().filter_map(|(system_id, system)| {
                    if system.flags().contains(&SystemFlag::Blocking) || system.flags().is_empty() {
                        Some((system_id.clone(), system))
                    } else {
                        None
                    }
                });
    
                let to_execute = system_ptrs
                    .filter(|(s, _)| !current_interrupts.contains(s))
                    .filter(|(_, s)| s.wake_up(current_events.events()))
                    .filter(|(_, s)| s.test_criteria(&resource_keys))
                    .filter(|(_, s)| !blacklist.check_blocked(s.cached_accesses().scheduler()))
                    .collect::<Vec<_>>();
    
                let running_systems = to_execute.iter().map(|(system_id, _)| system_id.clone()).collect::<HashSet<_>>();

                (if to_execute.len() > self.threadpool.max_count() {
                    let independent_systems = Self::lift_independent(to_execute.into_iter())
                        .map(|systems| {
                            systems.into_iter().map(|(stored_system, system_id)| {
                                (system_id, stored_system.ordering())
                            }).collect::<Vec<_>>()
                        });
                    
                    independent_systems
                        .into_iter()
                        .map(|systems| tokio::sync::RwLock::new(ExecutionGraph::new(&systems)))
                        .collect()
                } else {
                    let systems = to_execute.into_iter().map(|(system_id, stored_system)| {
                        (system_id, stored_system.ordering())  
                    }).collect::<Vec<_>>();
    
                    vec![
                        tokio::sync::RwLock::new(ExecutionGraph::new(&systems))
                    ]
                }, running_systems)
            };

            if !execution_graphs.is_empty() {
                for system in running_systems {
                    self.insert_new_event::<Id>(system.into())
                }

                let (systems, errors) = Self::execute_graphs(
                    self.systems.take().unwrap(),
                    &self.resources,
                    &self.system_resources,
                    &self.ids,
                    Arc::new(execution_graphs),
                    &self.background_accesses,
                    &self.threadpool
                );

                for (system_id, error) in errors {
                    event!(Level::WARN, system_id = ?system_id, error = %error);
                }

                let _ = self.systems.insert(systems);
            }

            let resource_guard = self.resources.read();
            resource_guard.resolve::<Unique<Blacklists>>().unwrap().get_mut(&phase).unwrap().tick();
        }

        event!(Level::INFO, phase = ?Phase::BackgroundEnd, "Executing Scheduler Phase: {:?}", Phase::BackgroundEnd);
        let mut retain = Vec::new();
        for (system_id, join_handle) in self.current_background_systems.drain(..).collect::<Vec<_>>() {
            if join_handle.is_finished() {
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

        {
            event!(Level::INFO, phase = ?Phase::BackgroundStart, "Executing Scheduler Phase: {:?}", Phase::BackgroundStart);
            let to_execute = {
                let resource_guard = self.resources.read();
                
                let mut current_events = resource_guard.resolve::<Unique<CurrentEvents>>().unwrap();
                current_events.insert(SchedulerEvent::from(Id::from(&Phase::BackgroundStart)));
                    
                let mut current_interrupts = resource_guard.resolve::<Unique<CurrentInterrupts>>().unwrap();
                current_interrupts.extend(self.current_background_systems.iter().map(|(system_id, _)| system_id.clone()));
                        
                let blacklist = resource_guard.resolve::<Shared<Blacklists>>().unwrap().get(&Phase::BackgroundStart).unwrap();
                let resource_keys = resource_guard.keys().collect();
                let system_ptrs = self.systems.as_ref().unwrap().iter().filter_map(|(system_id, system)| {
                    if system.flags().contains(&SystemFlag::Blocking) {
                        Some((system_id.clone(), system))
                    } else {
                        None
                    }
                });
        
                system_ptrs
                    .filter(|(s, _)| !current_interrupts.contains(s))
                    .filter(|(_, s)| s.wake_up(current_events.events()))
                    .filter(|(_, s)| s.test_criteria(&resource_keys))
                    .filter(|(_, s)| !blacklist.check_blocked(s.cached_accesses().scheduler()))
                    .map(|(system_id, _)| system_id)
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
                                        system.cached_accesses().conflicts(access)
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
                                                    sys.run(
                                                        &scheduler_resource_map, 
                                                        Some(&system_resource_map),
                                                        system_id, 
                                                        ids, 
                                                        None
                                                    ).unwrap();
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
            event!(Level::INFO, phase = ?Phase::Movement, "Executing Scheduler Phase: {:?}", Phase::Movement);
            let resources = self.resources.read();
            let new_resources = resources.resolve::<Unique<NewResources>>().unwrap().write().drain().collect::<HashMap<_, _>>();
            for (system_id, resource_map) in new_resources {
                if let Some(system_id) = system_id {
                    let _ = self.system_resources.get(&system_id).unwrap().conservatively_merge(resource_map);
                } else {
                    let _ = resources.conservatively_merge(resource_map);
                }
            }
        }
    }

    pub fn lift_independent<'a, T>(systems: T) -> impl Iterator<Item = Vec<(&'a StoredSystem, SystemId)>> 
        where T: Iterator<Item = (SystemId, &'a StoredSystem)>
    {
        let mut independent: Vec<HashSet<SystemId>> = Vec::new();
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
        mut systems: HashMap<SystemId, StoredSystem>,
        scheduler_resource_map: &Arc<parking_lot::RwLock<ResourceMap>>,
        system_resource_maps: &Arc<HashMap<SystemId, Arc<SystemResource>>>,
        ids: &Arc<RwLock<HashSet<String>>>,
        execution_graphs: Arc<Vec<tokio::sync::RwLock<ExecutionGraph<SystemId>>>>,
        accesses: &Arc<tokio::sync::RwLock<HashMap<SystemId, AccessMap>>>,
        threadpool: &threadpool::ThreadPool,
    ) -> (HashMap<SystemId, StoredSystem>, Vec<(SystemId, anyhow::Error)>) {
        let errors = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        let graph_count = execution_graphs.len();
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

        let inner_stored_systems: Arc<HashMap<_, _>> = Arc::new(systems.iter_mut().map(|(system_id, system)| {
            (system_id.clone(), SystemCell::new(system.take_system().unwrap()))
        }).collect());


        let hollow_systems = Arc::new(systems);

        for i in 0..threadpool.max_count() {
            // trial-and-errored this formula in rust playground
            let start_graph = (i * graph_count) / threadpool.max_count();

            let finished = Arc::clone(&finished);
            let execution_graphs = Arc::clone(&execution_graphs);
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
                            let current_graph = execution_graphs.get(current_graph_index).unwrap();

                            // chain is the count of how many systems were "passed"
                            //  reasons: another thread is handling it, there are access conflicts, its pending and not done
                            let mut chain = 0;

                            // not sure if i could store the leaf count or if that could lead to data races
                            'graphs_walk: while !current_graph.read().await.finished().load(Ordering::Acquire) && chain <= ( 2 * current_graph.read().await.leaves().count()) {
                                let leaf_count = current_graph.read().await.leaves().count();
                                if leaf_count > 0 {
                                    let nth_leaf = if let Some((system_id, status)) = current_graph.read().await.leaves().nth(chain % leaf_count) {
                                        if status.load(Ordering::Acquire) == ExecutionGraph::<SystemId>::PENDING {
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
                                                                system.cached_accesses().conflicts(access)
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
                                                                if let Err(err) = system.run(
                                                                    &scheduler_resource_map, 
                                                                    system_resource_map_ptrs.get(&system_id), 
                                                                    system_id.clone(), 
                                                                    Arc::clone(&ids), 
                                                                    Some(&system_resource_maps)
                                                                ) {
                                                                    errors.lock().await.push((system_id.clone(), err));
                                                                }

                                                                *status = SystemStatus::Executed;
                                                                current_graph.write().await.mark_as_complete(&system_id);
                                                                accesses.write().await.remove(&system_id);
                                                            },
                                                            InnerStoredSystem::Async(system) => {
                                                                let mut task = system.run(
                                                                    &scheduler_resource_map, 
                                                                    system_resource_map_ptrs.get(&system_id), 
                                                                    system_id.clone(), 
                                                                    Arc::clone(&ids), 
                                                                    Some(&system_resource_maps)
                                                                );

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
                                                                        if let Err(err) = result {
                                                                            errors.lock().await.push((system_id.clone(), err));
                                                                        }

                                                                        current_graph.write().await.mark_as_complete(&system_id);
                                                                        *status = SystemStatus::Executed;

                                                                        accesses.write().await.remove(&system_id);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        
                                                        assert_ne!(*status, SystemStatus::Executing);
                                                    },
                                                    SystemStatus::Pending => chain += 1,
                                                    SystemStatus::Executed => { /* Somehow possible but is benign :) */ },
                                                    SystemStatus::Executing => { unreachable!("Somehow got a lock while another thread should be holding it") }
                                                }
                                            }
                                            Err(err) => {
                                                assert!(matches!(err, TryLockError::WouldBlock), "How poison?");

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
                                            if let Err(err) = result {
                                                errors.lock().await.push((system_id.clone(), err));
                                            }

                                            let system = systems.get(&system_id).unwrap();

                                            *system.status().lock().unwrap() = SystemStatus::Executed;
                                            execution_graphs.get(graph_number).unwrap().write().await.mark_as_complete(&system_id);
                                            accesses.write().await.remove(&system_id);
                                        }
                                    }
                                }

                                tasks.extend(not_done);
                            }

                            current_graph_index = ( current_graph_index + 1 ) % graph_count;
                        }
                    });
                }
            )
        }

        threadpool.join();

        let mut systems = Arc::try_unwrap(hollow_systems).unwrap();

        for (system_id, system_cell) in Arc::try_unwrap(inner_stored_systems).unwrap() {
            let system = systems.get_mut(&system_id).unwrap();
            *system.status().lock().unwrap() = SystemStatus::Init;
            system.insert_system(system_cell.consume());
        }


        (systems, Arc::try_unwrap(errors).unwrap().into_inner())
    }
}