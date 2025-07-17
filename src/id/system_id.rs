use crate::id::Id;


#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct SystemId(Id);

impl From<Id> for SystemId {
    fn from(value: Id) -> Self {
        Self(value)
    }
}

impl From<SystemId> for Id {
    fn from(value: SystemId) -> Self {
        value.0
    }
}