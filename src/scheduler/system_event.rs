#[derive(Debug)]
pub enum SystemEvent {
    /// Ensures the scheduler removes the system event from new events
    NoSystemEvent,
    /// Inserts a <_>Failed event. Where <_> is what the event of the system
    // FailSystemEvent,
    /// Inserts a <1><2> event. Where <1> is what the event of the system, <2> is the String
    SignalEvent(String),
    /// Same as SignalEvent but using the "delay" version of the scheduler public API
    DelayedSignalEvent(String, usize),
    /// Inserts the string as an event, equivalent of inserting Id::from(String.as_str()) as a new event
    Event(String),
    /// Same as DelayedSignalEvent but using Event semantics
    DelayedEvent(String, usize),
}

#[derive(Debug)]
pub enum SystemResult {
    // Success does nothing
    Success,
    SystemEvent(SystemEvent),
    // panics with the error
    Error(anyhow::Error)
}