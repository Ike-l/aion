use std::any::type_name;

use crate::{parameters::InjectionParam, scheduler::{accesses::{access::Access, access_map::AccessMap}, resources::resource_map::ResourceMap}};


#[derive(small_derive_deref::Deref, Debug)]
pub struct Shared<'a, T> {
    value: &'a T
}

impl<'a, T: 'static> Shared<'a, T> {
    pub fn new(value: &'a T) -> Self {
        Self {
            value
        }
    }
}

impl<T: 'static> InjectionParam for Shared<'_, T> {
    type Item<'new> = Shared<'new, T>;
    
    fn failed_message() -> String {
        format!("Expected Resource: `{}`", type_name::<T>())
    }
    
    fn resolve_accesses(accesses: &mut AccessMap) {
        Self::access::<T>(accesses, Access::Shared)
    }
        
    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        Some( Shared::new( Self::try_typed_retrieve::<T>( resource_map )? ) )
    }
}