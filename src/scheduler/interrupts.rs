use std::collections::HashSet;

use crate::id::system_id::SystemId;

pub struct NewInterrupts {
    interrupts: HashSet<SystemId>
}

impl NewInterrupts {
    pub fn insert(&mut self, interrupt: SystemId) -> bool {
        self.interrupts.insert(interrupt)
    }
}

pub struct CurrentInterrupts {
    interrupts: HashSet<SystemId>
}

impl CurrentInterrupts {
    pub fn tick(&mut self, new_interrupts: &mut NewInterrupts) -> &mut Self {
        self.interrupts.clear();

        for interrupt in new_interrupts.interrupts.drain() {
            self.interrupts.insert(interrupt);
        }

        self
    }

    pub fn contains(&self, system_id: &SystemId) -> bool {
        self.interrupts.contains(system_id)
    }
}

impl Extend<SystemId> for CurrentInterrupts {
    fn extend<T: IntoIterator<Item = SystemId>>(&mut self, iter: T) {
        self.interrupts.extend(iter);
    }
}