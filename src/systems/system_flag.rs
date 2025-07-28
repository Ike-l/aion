
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SystemFlag {
    // Sync/Async
    Blocking,
    // Background Threads
    NonBlocking,
    // If fails criteria - will panic
    HasRequirements,
    // If is blocked by a blacklist
    NotBlacklisted
}