use std::hash::{DefaultHasher, Hash, Hasher};

use crate::scheduler::phase::Phase;

pub mod system_id;
pub mod event_id;

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Id(u64);

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        Self(hasher.finish())
    }
}

impl From<&Phase> for Id {
    fn from(phase: &Phase) -> Self {
        let str = format!("{:?}", phase);
        Self::from(str.as_str())
    }
}
