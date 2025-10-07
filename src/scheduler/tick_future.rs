use std::{sync::atomic::Ordering, task::Poll};

use threadpool::ThreadPool;

use crate::{id::Id, scheduler::execution_graph::panic_safe_graphs::PanicSafeGraphs};

pub struct TickFuture<'a> {
    pub threadpool: &'a ThreadPool,
    pub execution_graphs: &'a PanicSafeGraphs<Id>
}

impl<'a> Future for TickFuture<'a> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if self.execution_graphs.drop_signals.load(Ordering::Relaxed) >= self.threadpool.max_count() {
            if self.execution_graphs.panicked_signal.load(Ordering::Relaxed) {
                panic!("A Scheduler thread has panicked. Choosing to panic the main thread");
            } else {
                self.threadpool.join();      
                Poll::Ready(())
            }            
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}