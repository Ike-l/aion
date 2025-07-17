use std::any::type_name;

use crate::{parameters::{InjectionParam, Target}, scheduler::{accesses::access_map::AccessMap, resources::resource_map::ResourceMap}};

#[derive(small_derive_deref::Deref, small_derive_deref::DerefMut, Debug)]
pub struct Local<'a, T: InjectionParam> {
    res: T::Item<'a>,
}

impl<'a, T: InjectionParam> Local<'a, T> {
    pub fn new(res: T::Item<'a>) -> Self {
        Self {
            res,
        }
    }
}

impl<T: InjectionParam> InjectionParam for Local<'_, T> {
    type Item<'new> = Local<'new, T>;
    
    fn failed_message() -> String {
        format!("Expected Local InjectionParam: `{}`. Failed with {}", type_name::<T>(), T::failed_message())
    }
    
    fn resolve_accesses(accesses: &mut AccessMap) {
        T::resolve_accesses(accesses);
    }
    
    fn select_target() -> Target {
        Target::System
    }
    
    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>> {
        Some( Local::new( unsafe { T::try_retrieve(resource_map)? } ) )
    }
}