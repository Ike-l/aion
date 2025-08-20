use std::collections::HashMap;

use crate::{id::Id, scheduler::resources::resource_map::ResourceMap};


#[derive(Debug, Default)]
pub struct NewResources {
    resources: parking_lot::RwLock<HashMap<Option<Id>, ResourceMap>>
}

impl NewResources {
    pub fn insert(&self, system_id: Option<Id>, resource_map: ResourceMap) -> anyhow::Result<()> {
        self.resources.write().entry(system_id).or_default().conservatively_merge(resource_map)
    }

    pub fn write(&mut self) -> parking_lot::RwLockWriteGuard<HashMap<Option<Id>, ResourceMap>> {
        self.resources.write()
    }
}
