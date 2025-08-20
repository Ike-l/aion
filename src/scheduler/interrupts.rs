use std::collections::HashSet;

use crate::id::Id;

#[derive(Debug, Default)]
pub struct NewInterrupts {
    interrupts: HashSet<Id>
}

impl NewInterrupts {
    pub fn insert<T: Into<Id>>(&mut self, interrupt: T) -> bool {
        self.interrupts.insert(interrupt.into())
    }
}

#[derive(Debug, Default)]
pub struct CurrentInterrupts {
    interrupts: HashSet<Id>
}

impl CurrentInterrupts {
    pub fn tick(&mut self, new_interrupts: &mut NewInterrupts) -> &mut Self {
        self.interrupts.clear();

        for interrupt in new_interrupts.interrupts.drain() {
            self.interrupts.insert(interrupt);
        }

        self
    }

    pub fn contains(&self, system_id: &Id) -> bool {
        self.interrupts.contains(system_id)
    }
}

impl Extend<Id> for CurrentInterrupts {
    fn extend<T: IntoIterator<Item = Id>>(&mut self, iter: T) {
        self.interrupts.extend(iter);
    }
}