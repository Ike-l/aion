use crate::scheduler::tick::Tick;

#[derive(Debug, Clone)]
pub struct Lifetime {
    pub start: Tick,
    pub age: Tick,
    pub expected_age: Option<Tick>
}

impl Lifetime {
    pub fn new(start: Tick, expected_age: Tick) -> Self {
        Self {
            start,
            age: Tick::default(),
            expected_age: Some(expected_age)
        }
    }

    pub fn new_perpetual(start: Tick) -> Self {
        Self {
            start,
            age: Tick::default(),
            expected_age: None
        }
    }

    /// bool: true: keep alive, false: can die
    pub fn tick(&mut self) -> bool {
        self.age.0 += 1;
        if let Some(expected_age) = &self.expected_age {
            // <= : expected_age: 1 will survive pre-emptive tick
            return self.age <= *expected_age
        } else {
            return true
        }
    }
}