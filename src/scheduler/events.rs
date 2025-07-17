use std::collections::HashSet;

use crate::id::event_id::SchedulerEvent;

pub struct NewEvents {
    events: HashSet<SchedulerEvent>
}

impl NewEvents {
    pub fn insert(&mut self, event: SchedulerEvent) -> bool {
        self.events.insert(event)
    }
}

pub struct CurrentEvents {
    events: HashSet<SchedulerEvent>
}

impl CurrentEvents {
    pub fn tick(&mut self, new_events: &mut NewEvents) -> &mut Self {
        self.events.clear();

        for event in new_events.events.drain() {
            self.events.insert(event);
        }

        self
    }

    pub fn insert(&mut self, event: SchedulerEvent) {
        self.events.insert(event);
    }

    pub fn events(&self) -> &HashSet<SchedulerEvent> {
        &self.events
    }
}