use crate::id::Id;

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct SchedulerEvent(Id);

impl From<Id> for SchedulerEvent {
    fn from(value: Id) -> Self {
        Self(value)
    }
}

impl From<SchedulerEvent> for Id {
    fn from(value: SchedulerEvent) -> Self {
        value.0
    }
}