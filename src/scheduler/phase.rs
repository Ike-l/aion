// Order defined here is order scheduler does
#[derive(small_iter_fields::IterFields, small_iter_fields::HashFields, Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub enum Phase {
    Startup,
    Executing,
    Finishing,
    BackgroundEnd,
    BackgroundStart,
    Movement
}