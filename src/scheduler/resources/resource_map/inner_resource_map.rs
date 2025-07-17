use std::{any::TypeId, cell::UnsafeCell, collections::HashMap};

use crate::scheduler::resources::Resource;

#[derive(Debug, Default)]
pub(crate) struct InnerResourceMap {
    resources: UnsafeCell<HashMap<TypeId, Resource>>
}

unsafe impl Send for InnerResourceMap {}
unsafe impl Sync for InnerResourceMap {}

impl InnerResourceMap {
    /// Ensure unique access through transitive locks
    pub(crate) fn get_map_mut<'a>(&'a self, lock: &'a parking_lot::RwLock<()>) -> (&'a mut HashMap<TypeId, Resource>, parking_lot::RwLockWriteGuard<'a, ()>) {
        (unsafe { &mut *self.resources.get() }, lock.write())
    }

    pub(crate) fn get_map<'a>(&'a self, lock: &'a parking_lot::RwLock<()>) -> (&'a HashMap<TypeId, Resource>, parking_lot::RwLockReadGuard<'a, ()>) {
        (unsafe { & *self.resources.get() }, lock.read())
    }

    pub(crate) fn consume(self) -> HashMap<TypeId, Resource> {
        self.resources.into_inner()
    }
}