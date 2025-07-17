use std::hash::{DefaultHasher, Hash, Hasher};

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
