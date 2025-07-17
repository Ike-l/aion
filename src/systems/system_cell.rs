use std::{cell::UnsafeCell, fmt::Debug};

use crate::systems::stored_system::inner_stored_system::InnerStoredSystem;

#[derive(Debug)]
pub struct SystemCell {
    pub system: UnsafeCell<InnerStoredSystem>
}

impl SystemCell {
    pub fn new(system: InnerStoredSystem) -> Self {
        Self {
            system: UnsafeCell::new(system)
        }
    }

    pub fn consume(self) -> InnerStoredSystem {
        self.system.into_inner()
    }
}

unsafe impl Send for SystemCell {}
unsafe impl Sync for SystemCell {}

