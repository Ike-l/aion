use std::collections::HashSet;

use crate::id::Id;

#[derive(Debug, Default)]
pub struct NewEvents {
    events: HashSet<Id>,
    in_use: parking_lot::Mutex<()>,
}

impl NewEvents {
    pub fn insert(&mut self, event: Id) -> bool {
        let _guard = self.in_use.lock();
        self.events.insert(event)
    }

    pub fn remove(&mut self, event: Id) {
        let _guard = self.in_use.lock();
        self.events.remove(&event);
    }
}

#[derive(Debug, Default)]
pub struct CurrentEvents {
    events: parking_lot::RwLock<HashSet<Id>>
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

    pub fn insert<T: Into<Id>>(&mut self, event: T) {
        self.events.write().insert(event.into());
    }

    pub fn remove(&mut self, to_remove: &Id) {
        self.events.write().retain(|event| event != to_remove);
    }

    pub fn events(&self) -> parking_lot::RwLockReadGuard<HashSet<Id>> {
        self.events.read()
    }
}