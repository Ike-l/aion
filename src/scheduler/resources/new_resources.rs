use std::collections::HashMap;

use crate::{id::system_id::SystemId, scheduler::resources::resource_map::ResourceMap};


#[derive(Debug, Default)]
pub struct NewResources {
    resources: parking_lot::RwLock<HashMap<Option<SystemId>, ResourceMap>>
}

impl NewResources {
    pub fn insert(&self, system_id: Option<SystemId>, resource_map: ResourceMap) -> anyhow::Result<()> {
        self.resources.write().entry(system_id).or_default().conservatively_merge(resource_map)
    }

    pub fn write(&mut self) -> parking_lot::RwLockWriteGuard<HashMap<Option<SystemId>, ResourceMap>> {
        self.resources.write()
    }
}
