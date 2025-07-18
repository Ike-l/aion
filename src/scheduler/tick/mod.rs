pub mod lifetime;
pub mod current_tick;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct Tick(pub usize);
