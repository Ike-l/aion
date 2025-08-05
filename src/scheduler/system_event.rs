use std::fmt::Display;

#[derive(Debug)]
pub enum SystemEvent {
    /// Ensures the scheduler removes the system event from new events
    NoSystemEvent,
    /// Inserts a <_>Failed event. Where <_> is what the event of the system
    // FailSystemEvent,
    /// Inserts a <1><2> event. Where <1> is what the event of the system, <2> is the String
    SignalEvent(String),
}

impl Display for SystemEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scheduler Event")        
    }
}