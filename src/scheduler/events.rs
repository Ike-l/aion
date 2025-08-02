use std::collections::HashSet;

use crate::id::event_id::SchedulerEvent;

#[derive(Debug, Default)]
pub struct NewEvents {
    events: HashSet<SchedulerEvent>,
    in_use: parking_lot::Mutex<()>,
}

impl NewEvents {
    pub fn insert(&mut self, event: SchedulerEvent) -> bool {
        let _guard = self.in_use.lock();
        self.events.insert(event)
    }
}

#[derive(Debug, Default)]
pub struct CurrentEvents {
    events: parking_lot::RwLock<HashSet<SchedulerEvent>>
}

impl CurrentEvents {
    pub fn tick(&mut self, new_events: &mut NewEvents) -> &mut Self {
        let mut events = self.events.write();
        events.clear();

        for event in new_events.events.drain() {
            events.insert(event);
        }

        drop(events);

        self
    }

    pub fn insert(&mut self, event: SchedulerEvent) {
        self.events.write().insert(event);
    }

    pub fn remove(&mut self, to_remove: &SchedulerEvent) {
        self.events.write().retain(|event| event != to_remove);
    }

    pub fn events(&self) -> parking_lot::RwLockReadGuard<HashSet<SchedulerEvent>> {
        self.events.read()
    }
}