use std::time::{Duration, Instant};

use crate::{parameters::injections::unique::Unique, scheduler::{system_event::SystemResult, tick::Tick}};

#[derive(Debug)]
pub struct CurrentTick {
    pub tick: Tick,
    pub dt: Duration,
    pub time: Instant,
}

impl Default for CurrentTick {
    fn default() -> Self {
        Self {
            tick: Tick(0),
            dt: Duration::default(),
            time: Instant::now(),
        }
    }
}

// it is NOT an axiom that each tick the CurrentTick will be incremented by 1, Since other systems could theoretically increment it in Phase::Pre

// https://english.stackexchange.com/questions/507012/incrementor-vs-incrementer
// Phase::Pre
pub fn tick_incrementor(mut current_tick: Unique<CurrentTick>) -> Option<SystemResult> {
    let current_time = Instant::now();
    let dt = current_time.duration_since(current_tick.time);
    
    current_tick.tick.0 += 1;
    current_tick.dt = dt;
    current_tick.time = current_time;

    None
}
