use std::collections::HashMap;

use crate::scheduler::{blacklists::blacklist::Blacklist, phase::Phase};

pub mod blacklist;

#[derive(Debug, small_derive_deref::Deref, small_derive_deref::DerefMut, Clone)]
pub struct Blacklists {
    pub blacklists: HashMap<Phase, Blacklist>
}

impl Default for Blacklists {
    fn default() -> Self {
        Self {
            blacklists: Phase::to_hashmap(Blacklist::default())
        }
    }
}

