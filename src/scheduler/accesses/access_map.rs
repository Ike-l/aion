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
}

impl<const N: usize> From<[(TypeId, Access); N]> for AccessMap {
    fn from(value: [(TypeId, Access); N]) -> Self {
        Self {
            accesses: HashMap::from(value)
        }
    }
}
