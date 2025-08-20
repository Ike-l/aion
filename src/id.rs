use std::{collections::HashMap, hash::{DefaultHasher, Hash, Hasher}};

use crate::scheduler::phase::Phase;

#[derive(Debug, Clone, PartialOrd, Ord, Eq)]
pub struct Id {
    id: u64,
    generation: Option<u64>
}

impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && match (self.generation, other.generation) {
            (Some(g1), Some(g2)) => g1 == g2,
            _ => true
        }
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        match self.generation {
            Some(g) => g.hash(state),
            None => ().hash(state),    
        }
    }
}


impl Id {
    pub fn new<T: Hash>(id: T, generations: &mut HashMap<u64, u64>) -> Self {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let id = hasher.finish();

        let generation = generations.entry(id).or_default();
        *generation += 1;

        Self {
            id,
            generation: Some(*generation)
        }
    }

    pub fn id(&self) -> &u64 {
        &self.id
    }
}

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        Self { id: hasher.finish(), generation: None }
    }
}

impl From<&Phase> for Id {
    fn from(phase: &Phase) -> Self {
        let str = format!("{:?}", phase);
        Self::from(str.as_str())
    }
}
