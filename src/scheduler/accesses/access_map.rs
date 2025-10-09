use std::{any::TypeId, collections::HashMap};

use crate::scheduler::accesses::access::Access;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessMap {
    pub accesses: HashMap<TypeId, Access>
}

impl AccessMap {
    pub fn new() -> Self {
        Self {
            accesses: HashMap::new()
        }
    }

    pub fn conflicts(&self, other: &AccessMap) -> bool {
        other.accesses.iter().any(|(ty, acc)| {
            if let Some(access) = self.accesses.get(ty) {
                !( *acc == Access::Shared && *access == Access::Shared )
            } else {
                false
            }
        })
    }
    
    pub fn merge(&mut self, other: impl Iterator<Item = (TypeId, Access)>) {
        for (id, access) in other {
            self.accesses.insert(id, access);
        }
    }
}

impl<const N: usize> From<[(TypeId, Access); N]> for AccessMap {
    fn from(value: [(TypeId, Access); N]) -> Self {
        Self {
            accesses: HashMap::from(value)
        }
    }
}
