use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc};

use crate::scheduler::execution_graph::ExecutionGraph;

pub struct PanicSafeGraphs<T> {
    pub drop_signal: Arc<AtomicUsize>,
    pub panicked_signal: Arc<AtomicBool>,
    pub graphs: Arc<Vec<tokio::sync::RwLock<ExecutionGraph<T>>>>
}

impl<T> PanicSafeGraphs<T> {
    pub fn new(graphs: Arc<Vec<tokio::sync::RwLock<ExecutionGraph<T>>>>) -> Self {
        Self {
            drop_signal: Arc::new(AtomicUsize::new(0)),
            panicked_signal: Arc::new(AtomicBool::new(false)),
            graphs
        }
    }

    pub fn arc_clone(&self) -> Self {
        Self {
            drop_signal: Arc::clone(&self.drop_signal),
            panicked_signal: Arc::clone(&self.panicked_signal),
            graphs: Arc::clone(&self.graphs)
        }
    }
}

impl<T> Drop for PanicSafeGraphs<T> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.panicked_signal.store(true, Ordering::SeqCst);
        } else {
            self.drop_signal.fetch_add(1, Ordering::SeqCst);
        }
    }
}