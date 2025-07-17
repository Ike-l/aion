use std::{any::TypeId, collections::HashSet};

use crate::{parameters::{InjectionParam, Target}, scheduler::{accesses::access_map::AccessMap, resources::resource_map::ResourceMap}};

#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct Optional<'a, T: InjectionParam> {
    inner: Option<T::Item<'a>>
}

impl<'a, T: InjectionParam> Optional<'a, T> {
    pub fn new(inner: Option<T::Item<'a>>) -> Self {
        Self {
            inner
        }
    }
}

impl<T: InjectionParam> InjectionParam for Optional<'_, T> {
    type Item<'new> = Optional<'new, T>;
    
    fn failed_message() -> String {
        unreachable!()
    }
    
    fn resolve_accesses(accesses: &mut AccessMap) {
        T::resolve_accesses(accesses);
    }

    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        Some(Optional::new(unsafe { T::try_retrieve(resource_map) } ))
    }

    fn criteria(_owned_resources: &HashSet<&TypeId>) -> bool {
        true
    }

    fn select_target() -> Target {
        T::select_target()
    }
}

