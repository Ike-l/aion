use std::{collections::HashMap, hash::{DefaultHasher, Hash, Hasher}};

use crate::scheduler::phase::Phase;

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq, Hash)]
pub struct Id {
    id: u64,
    // generation: Option<u64>
}

// impl PartialEq for Id {
//     fn eq(&self, other: &Self) -> bool {
//         self.id == other.id 
//         && match (self.generation, other.generation) {
//             (Some(g1), Some(g2)) => g1 == g2,
//             (_, None) => false,
//             _ => true,
//         }
//     }
// }

// impl Hash for Id {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         self.id.hash(state);
//         // match self.generation {
//         //     Some(g) => g.hash(state),
//         //     None => ().hash(state),    
//         // }
//     }
// }


impl Id {
    pub fn new<T: Hash>(id: T, generations: &mut HashMap<u64, u64>) -> Self {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let id = hasher.finish();

        let generation = generations.entry(id).or_default();
        *generation += 1;

        Self {
            id,
            // generation: Some(*generation)
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
        Self { 
            id: hasher.finish(),
            // generation: None
        }
    }
}

impl From<&Phase> for Id {
    fn from(phase: &Phase) -> Self {
        let str = format!("{:?}", phase);
        Self::from(str.as_str())
    }
}

#[cfg(test)]
mod tests {
    // use std::collections::HashSet;

    use super::*;

    #[test]
    fn id_partial_eq() {
        let id0 = Id {
            id: 1,
            // generation: Some(1)
        };

        let id1 = Id {
            id: 1,
            // generation: Some(1)
        };

        let id2 = Id {
            id: 1,
            // generation: None
        };

        assert_eq!(id0, id1);

        // assert_ne!(id0, id2);
        assert_eq!(id2, id0);
    }

    // #[test]
    // fn id_hash_replacer() {
    //     let mut set = HashSet::new();

    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: Some(1)
    //         }
    //     );

    //     // This `None` sees the `Some` above and concludes the entry already exists
    //     // so doesnt insert
    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: None
    //         }
    //     );

    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: Some(2)
    //         }
    //     );

    //     assert_eq!(set.len(), 2);
    // }

    // #[test]
    // fn id_hash_adder() {
    //     let mut set = HashSet::new();

    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: None
    //         }
    //     );

    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: Some(1)
    //         }
    //     );

    //     set.insert(
    //         Id {
    //             id: 1,
    //             generation: Some(2)
    //         }
    //     );

    //     assert_eq!(set.len(), 3);
    // }
}