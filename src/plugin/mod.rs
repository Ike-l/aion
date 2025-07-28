use crate::scheduler::Scheduler;

pub trait Plugin {
    fn plugin(&self, scheduler: &mut Scheduler);
}