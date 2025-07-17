use std::sync::atomic::AtomicBool;

use anyhow::Context;

use crate::scheduler::resources::resource_map::ResourceMap;

pub mod system_resource_ptr;

#[derive(Debug, Default)]
pub struct SystemResource {
    in_use: AtomicBool,
    resources: ResourceMap,
    in_use_notify: tokio::sync::Notify,
}

impl SystemResource {
    pub fn conservatively_merge(&self, other: ResourceMap) -> anyhow::Result<()> {
        self.resources.conservatively_merge(other).context("From SystemResource")
    }
}

