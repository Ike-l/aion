pub mod sync_system;
pub mod async_system;
pub mod stored_system;
pub mod system_flag;
pub mod system_status;
pub mod system_cell;

pub struct FunctionSystem<Input, F> {
    f: F,
    marker: std::marker::PhantomData<fn() -> Input>
}