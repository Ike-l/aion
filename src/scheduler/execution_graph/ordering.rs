use std::collections::HashSet;

use crate::id::system_id::SystemId;

pub trait ExecutionOrdering {
    type Item;

    fn subsume(&self, superset: &HashSet<Self::Item>) -> Self;
    /// before | after | priority
    fn consume(self) -> ( HashSet<Self::Item>, HashSet<Self::Item>, f64 );
}

/// "Before": This node is "Before" everything in this HashSet<SystemId>
/// Not to be confused with "Before": Everything in this HashSet<SystemId> is before this node
#[derive(Debug, Default, Clone)]
pub struct SchedulerOrdering {
    pub before: HashSet<SystemId>,
    pub after: HashSet<SystemId>,
    pub priority: f64
}

impl ExecutionOrdering for SchedulerOrdering {
    type Item = SystemId;

    fn subsume(&self, superset: &HashSet<Self::Item>) -> Self {
        Self {
            before: self.before.intersection(superset).cloned().collect(),
            after: self.after.intersection(superset).cloned().collect(),
            priority: self.priority
        }
    }

    fn consume(self) -> ( HashSet<Self::Item>, HashSet<Self::Item>, f64 ) {
        ( self.before, self.after, self.priority )
    }
}