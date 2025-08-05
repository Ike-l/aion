use crate::scheduler::Scheduler;

pub trait Plugin {
    /// Ensure plugin is called before the first `tick`
    fn plugin(&self, scheduler: &mut Scheduler);
}