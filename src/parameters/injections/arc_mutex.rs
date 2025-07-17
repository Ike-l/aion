use std::sync::{Arc, Mutex};

use crate::{parameters::{injections::shared::Shared, InjectionParam}, scheduler::{accesses::{access::Access, access_map::AccessMap}, resources::resource_map::ResourceMap}};

#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct ArcMutex<T: 'static> {
    inner: Arc<tokio::sync::Mutex<T>>
}

impl<T: 'static> ArcMutex<T> {
    pub fn new(a: &Arc<tokio::sync::Mutex<T>>) -> Self {
        Self {
            inner: Arc::clone(a)
        }
    }
}

impl<T: 'static> InjectionParam for ArcMutex<T> {
    type Item<'new> = Self;

    fn failed_message() -> String {
        Shared::<Arc<Mutex<T>>>::failed_message()
    }

    fn resolve_accesses(accesses: &mut AccessMap) {
        Self::access::<Arc<tokio::sync::Mutex<T>>>(accesses, Access::Shared);
    }

    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        Some(ArcMutex::new(resource_map.get::<Arc<tokio::sync::Mutex<T>>>()?))
    }
}
