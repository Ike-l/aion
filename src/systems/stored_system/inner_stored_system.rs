use std::{any::TypeId, collections::HashSet, fmt::Debug};

use crate::{scheduler::accesses::Accesses, systems::{async_system::{into_async::IntoAsyncSystem, AsyncSystem}, sync_system::{into_sync::IntoSyncSystem, SyncSystem}}};


pub enum InnerStoredSystem {
    Async(Box<dyn AsyncSystem>),
    Sync(Box<dyn SyncSystem>),
}

impl InnerStoredSystem {
    pub fn stringify_system_kind(&self) -> String {
        match self {
            Self::Async(_) => format!("Async"),
            Self::Sync(_) => format!("Sync")
        }
    }

    pub fn new_sync<T, S, I>(system: T) -> Self where T: IntoSyncSystem<I, System = S>, S: SyncSystem + 'static {
        Self::Sync(Box::new(system.into_system()))
    }

    pub fn new_async<T, S, I>(system: T) -> Self where T: IntoAsyncSystem<I, System = S>, S: AsyncSystem + 'static {
        Self::Async(Box::new(system.into_system()))
    }

    pub fn accesses(&self) -> Accesses {
        match self {
            Self::Async(s) => s.accesses(),
            Self::Sync(s) => s.accesses()
        }
    }

    pub fn needs_system_resource(&self) -> bool {
        match self {
            Self::Async(s) => s.needs_system_resource(),
            Self::Sync(s) => s.needs_system_resource()
        }
    }

    pub fn criteria(&self, resources: &HashSet<TypeId>) -> bool {
        match self {
            Self::Sync(s) => s.criteria(resources),
            Self::Async(s) => s.criteria(resources),
        }
    }
}

impl Debug for InnerStoredSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} Stored System", self.stringify_system_kind())
    }
}