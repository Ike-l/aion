use std::{any::TypeId, collections::HashSet, marker::PhantomData};

use crate::{parameters::{injections::shared::Shared, InjectionParam}, scheduler::{accesses::access_map::AccessMap, resources::resource_map::ResourceMap}};

/// T: Type you want
/// Y: Type you want to use
#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct Owned<T, Y> {
    #[DerefMutTarget]
    #[DerefTarget]
    value: T,
    _y: PhantomData<Y>
}

impl<T, Y: ToOwned<Owned = T>> Owned<T, Y> {
    pub fn new(value: &Y) -> Self {
        Self {
            value: value.to_owned(), 
            _y: PhantomData::default()
        }
    }

    pub fn consume(self) -> T {
        self.value
    }
}

impl<T, Y> InjectionParam for Owned<T, Y>
where
    Y: ToOwned<Owned = T> + 'static,
{
    type Item<'a> = Owned<T, Y>;

    fn failed_message() -> String {
        Shared::<Y>::failed_message()
    }

    fn resolve_accesses(_accesses: &mut AccessMap) {
        // No concurrent accesses since Owned (ToOwned)
    }

    fn criteria(owned_resources: &HashSet<&TypeId>) -> bool {
        owned_resources.contains(&TypeId::of::<Y>())
    }

    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        let reference: &Y = Self::try_typed_retrieve::<Y>(resource_map)?;
        Some(Owned::new(reference))
    }
}