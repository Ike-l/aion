use crate::{parameters::{injections::unique::Unique, InjectionParam}, scheduler::{accesses::{access::Access, access_map::AccessMap}, resources::resource_map::ResourceMap}};

#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct Take<T> {
    pub value: T
}

impl<T> Take<T> {
    pub fn new(value: T) -> Self {
        Self {
            value
        }
    }
}

impl<T: 'static + Default> InjectionParam for Take<T> {
    type Item<'new> = Take<T>;

    fn failed_message() -> String {
        Unique::<T>::failed_message()
    }

    fn resolve_accesses(accesses: &mut AccessMap) {
        Self::access::<T>(accesses, Access::Unique)
    }

    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        let reference: T = std::mem::take(Self::try_typed_mut_retrieve::<T>(resource_map)?);
        Some(Take::new(reference))
    }
}