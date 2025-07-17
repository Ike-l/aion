use std::any::TypeId;

use crate::scheduler::{accesses::{access::Access, access_map::AccessMap}, tick::lifetime::Lifetime};

#[derive(Debug, Default, Clone)]
pub struct Blacklist {
    access_blacklist: Vec<(Access, Lifetime)>,
    typed_blacklist: Vec<(TypeId, Access, Lifetime)>
}

impl Blacklist {
    pub fn check_blocked(&self, accesses: &AccessMap) -> bool {
        accesses.accesses.values().any(|access| {
            self.access_blacklist.iter().any(|(blocked_access, _)| access == blocked_access)
        }) || accesses.accesses.iter().any(|(typ, access)| {
            self.typed_blacklist.iter().any(|(blocked_type, blocked_access, _)| {
                typ == blocked_type && access == blocked_access
            })
        })
    }

    pub fn tick(&mut self) {
        self.access_blacklist.retain_mut(|(_, lifetime)| {
            lifetime.age.0 += 1;
            if let Some(expected_age) = &lifetime.expected_age {
                return lifetime.age < *expected_age
            } else {
                return true
            }
        });
    
        self.typed_blacklist.retain_mut(|(_, _, lifetime)| {
            lifetime.age.0 += 1;
            if let Some(expected_age) = &lifetime.expected_age {
                return lifetime.age < *expected_age
            } else {
                return true
            }
        });
    }
}

