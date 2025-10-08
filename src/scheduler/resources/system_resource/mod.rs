use std::{any::TypeId, sync::atomic::AtomicBool};

use crate::{scheduler::resources::{resource_map::ResourceMap, Resource}};

pub mod system_resource_ptr;

#[derive(Debug, Default)]
pub struct SystemResource {
    in_use: AtomicBool,
    resources: ResourceMap,
    in_use_notify: tokio::sync::Notify,
}

impl SystemResource {
    // pub fn conservatively_merge(&self, other: ResourceMap) -> anyhow::Result<()> {
    //     self.resources.conservatively_merge(other).context("From SystemResource")
    // }

    // pub fn conservatively_insert<T: 'static>(&self, type_id: TypeId, resource: T) -> anyhow::Result<()> {
    //     self.resources.conservatively_insert(type_id, resource)
    // }

    /// Safety:
    /// Ensure no reference alive when insert
    /// Use conservatively_insert for safety
    pub unsafe fn insert<T: 'static>(&mut self, type_id: TypeId, resource: T) -> Option<Resource> {
        unsafe { self.resources.insert(type_id, resource) }
    }

    // /// Safety:
    // /// Ensure `get` or `get_mut` safety
    // pub unsafe fn resolve<T: InjectionParam>(&self) -> Option<T::Item<'_>> {
    //     unsafe { T::try_retrieve(&self.resources) }
    // }
}

