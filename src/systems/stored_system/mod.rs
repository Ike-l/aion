use std::{any::TypeId, collections::HashSet, sync::Mutex};

use crate::{id::event_id::SchedulerEvent, scheduler::{accesses::Accesses, execution_graph::ordering::SchedulerOrdering}, systems::{stored_system::inner_stored_system::InnerStoredSystem, system_flag::SystemFlag, system_status::SystemStatus}};

pub mod inner_stored_system;

pub struct Criteria(pub Box<dyn Fn(&HashSet<SchedulerEvent>) -> bool + Send + Sync>);

impl std::fmt::Debug for Criteria {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Criteria")
    }
}

#[derive(Debug, small_read_only::ReadOnly)]
pub struct StoredSystem {
    system: Option<InnerStoredSystem>,
    wake_up_criteria: Criteria,
    display_name: String,
    ordering: SchedulerOrdering,
    status: Mutex<SystemStatus>,
    flags: HashSet<SystemFlag>,
    cached_accesses: Accesses,   
    needs_system_resource: bool,
}

impl StoredSystem {
    pub fn new(
        system: InnerStoredSystem,
        wake_up_criteria: Criteria,
        display_name: String,
        ordering: SchedulerOrdering,
        flags: HashSet<SystemFlag>
    ) -> Self {
        let cached_accesses = system.accesses();
        let needs_system_resource = system.needs_system_resource();
        Self {
            system: Some(system),
            wake_up_criteria,
            display_name,
            ordering,
            status: Mutex::new(SystemStatus::Init),
            flags,
            cached_accesses,
            needs_system_resource,
        }
    }

    pub fn wake_up(&self, events: &HashSet<SchedulerEvent>) -> bool {
        (self.wake_up_criteria.0)(events)
    }

    pub fn test_criteria(&self, resources: &HashSet<TypeId>) -> bool {
        self.system.as_ref().unwrap().criteria(resources)
    }

    pub fn take_system(&mut self) -> Option<InnerStoredSystem> {
        self.system.take()
    }

    pub fn insert_system(&mut self, system: InnerStoredSystem) -> &mut InnerStoredSystem {
        self.system.insert(system)
    }
}