use crate::scheduler::accesses::{access::Access, access_map::AccessMap};

pub mod access_map;
pub mod access;

#[derive(small_read_only::ReadOnly, Debug, PartialEq, Eq)]
pub struct Accesses {
    scheduler: AccessMap,
    // used when "run" to give the system resource map
    system: AccessMap
}

impl Accesses {
    pub fn new(scheduler: AccessMap, system: AccessMap) -> Self {
        Self {
            scheduler,
            system
        }
    }

    /// conflicts if its the accesses aren't both read 
    pub fn conflicts(&self, other: &AccessMap) -> bool {
        other.accesses.iter().any(|(ty, acc)| {
            if let Some(access) = self.scheduler.accesses.get(ty) {
                !( *acc == Access::Shared && *access == Access::Shared )
            } else {
                false
            }
        })
    }
}