use std::collections::HashSet;

use crate::id::Id;

pub trait ExecutionOrdering {
    type Item;

    fn subsume(&self, superset: &HashSet<Self::Item>) -> Self;
    /// before | after | priority
    fn consume(self) -> ( HashSet<Self::Item>, HashSet<Self::Item>, f64 );
}

/// "Before": This node is "Before" everything in this HashSet<Id>
/// Not to be confused with "Before": Everything in this HashSet<Id> is before this node
#[derive(Debug, Default, Clone)]
pub struct SchedulerOrdering {
    before: HashSet<Id>,
    after: HashSet<Id>,
    priority: f64
}

impl SchedulerOrdering {
    pub fn insert_before(mut self, system_id: Id) -> Self {
        self.before.insert(system_id);
        self
    }
    
    pub fn insert_after(mut self, system_id: Id) -> Self {
        self.after.insert(system_id);
        self
    }

    pub fn before(&self) -> &HashSet<Id> {
        &self.before
    }
    
    pub fn after(&self) -> &HashSet<Id> {
        &self.after
    }
}

impl ExecutionOrdering for SchedulerOrdering {
    type Item = Id;

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