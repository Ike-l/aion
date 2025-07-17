use std::{ops::Deref, sync::{atomic::Ordering, Arc}};

use crate::scheduler::resources::{resource_map::ResourceMap, system_resource::SystemResource};

pub struct SystemResourcePtr {
    system_resources: Arc<SystemResource>
}

impl SystemResourcePtr {
    pub fn new(system_resources: Arc<SystemResource>) -> Option<Self> {
        system_resources.in_use.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).ok()?;

        Some(Self {
            system_resources
        })
    }

    pub async fn new_async(system_resources: Arc<SystemResource>) -> Self {
        loop {
            if system_resources.in_use.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                return Self { system_resources }
            }

            system_resources.in_use_notify.notified().await
        }
    }
}

impl Deref for SystemResourcePtr {
    type Target = ResourceMap;

    fn deref(&self) -> &Self::Target {
        &self.system_resources.resources
    }
}

impl Drop for SystemResourcePtr {
    fn drop(&mut self) {
        self.system_resources.in_use.swap(false, Ordering::Release);
        self.system_resources.in_use_notify.notify_one();
    }
}