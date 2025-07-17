use crate::{parameters::{injections::shared::Shared, InjectionParam}, scheduler::{accesses::{access::Access, access_map::AccessMap}, resources::resource_map::ResourceMap}};

#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct Unique<'a, T> {
    value: &'a mut T
}

impl<'a, T: 'static> Unique<'a, T> {
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value
        }
    }
}

impl<T: 'static> InjectionParam for Unique<'_, T> {
    type Item<'new> = Unique<'new, T>;
    
    fn failed_message() -> String {
        Shared::<T>::failed_message()
    }
    
    fn resolve_accesses(accesses: &mut AccessMap) {
        Self::access::<T>(accesses, Access::Unique)
    }
    
    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        Some( Unique::new( Self::try_typed_mut_retrieve::<T>( resource_map )? ) )
    }
}