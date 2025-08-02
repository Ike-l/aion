// Order defined here is the actual order of the schedulers phases
#[derive(small_iter_fields::IterFields, small_iter_fields::HashFields, Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub enum Phase {
    Ticking,
    PreProcessing,
    Processing,
    PostProcessing,
    BackgroundEnd,
    BackgroundStart,
    Movement
}