use crate::scheduler::Scheduler;

pub trait Plugin {
    /// Safety:
    /// Ensure plugin is called before the first `tick`
    unsafe fn plugin(&self, scheduler: &mut Scheduler);
}