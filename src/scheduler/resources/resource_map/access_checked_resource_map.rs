use std::{collections::HashMap, sync::Arc};

use crate::{id::Id, parameters::InjectionParam, scheduler::{accesses::access_map::AccessMap, resources::resource_map::ResourceMap}};

// TODO track resources borrowed with resolve 
pub struct AccessCheckedResourceMap<'a> {
    resource_map: parking_lot::lock_api::RwLockReadGuard<'a, parking_lot::RawRwLock, ResourceMap>,
    accesses: &'a Arc<tokio::sync::RwLock<HashMap<Id, AccessMap>>>,
}

impl<'a> AccessCheckedResourceMap<'a> {
    pub fn new(resource_map: &'a Arc<parking_lot::RwLock<ResourceMap>>, accesses: &'a Arc<tokio::sync::RwLock<HashMap<Id, AccessMap>>>) -> Self {
        Self {
            resource_map: resource_map.read(),
            accesses
        }
    }

    pub fn resolve<T: InjectionParam + 'static>(&self) -> Result<Option<T::Item<'_>>, &'static str> {
        let mut accesses = AccessMap::default();
        T::resolve_accesses(&mut accesses);

        if !self.accesses.blocking_read().iter().any(|(_, access_map)| {
            accesses.conflicts(access_map)
        }) {
            // Safety:
            // Accesses are checked
            return Ok(unsafe {self.resource_map.resolve::<T>() });
        }

        Err("Access Denied")
    }
}