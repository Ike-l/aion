use std::{any::TypeId, collections::{HashMap, HashSet}, sync::{Arc, RwLock}};

use crate::{id::Id, parameters::{InjectionParam, Target}, scheduler::{accesses::{access_map::AccessMap, Accesses}, resources::{resource_map::ResourceMap, system_resource::{system_resource_ptr::SystemResourcePtr, SystemResource}}, system_event::SystemResult}, systems::FunctionSystem};

pub mod into_sync;

pub trait SyncSystem: Send + Sync {
    /// Safety:
    /// Ensure no concurrent mutable accesses via `fn accesses`
    unsafe fn run(
        &mut self,
        scheduler_resource_map: &ResourceMap,
        running_system_resource_map: Option<&SystemResourcePtr>,
        running_system_id: Id,
        ids: Arc<RwLock<HashMap<u64, String>>>,
        system_resource_maps: Option<&HashMap<Id, Arc<SystemResource>>>
    ) -> Option<SystemResult>;

    /// Does the scheduler have the resources the SystemParam needs?
    fn criteria(&self, owned_resources: &HashSet<TypeId>) -> bool;
    fn accesses(&self) -> Accesses;
    fn needs_system_resource(&self) -> bool;
}

macro_rules! impl_sync_system {
    (
        $($params:ident),*
    ) => {
        #[allow(clippy::too_many_arguments)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        impl<F, $($params: InjectionParam),*> SyncSystem for FunctionSystem<($($params,)*), F>
            where
                F: Send + Sync,
                for<'a, 'b> &'a mut F:
                    FnMut($($params),*) -> Option<SystemResult> +
                    FnMut($(<$params as InjectionParam>::Item<'b>),*) -> Option<SystemResult> 
        {
            unsafe fn run(
                &mut self,
                scheduler_resource_map: &ResourceMap,
                system_resource_map: Option<&SystemResourcePtr>,
                system_id: Id,
                id_map: Arc<RwLock<HashMap<u64, String>>>,
                system_resource_maps: Option<&HashMap<Id, Arc<SystemResource>>>,
            ) -> Option<SystemResult> {
                fn call_inner<$($params),*>(
                    mut f: impl FnMut($($params),*) -> Option<SystemResult>,
                    $($params: $params),*
                ) -> Option<SystemResult> {
                    f($($params),*)
                }

                $(
                    let $params = unsafe { $params::retrieve(
                        scheduler_resource_map,
                        system_resource_map,
                        system_id.clone(), 
                        id_map.read().unwrap(),
                        system_resource_maps, 
                    ).ok()? };
                )*

                drop(id_map);

                call_inner(&mut self.f, $($params),*)
            }
            
            fn criteria(&self, owned_resources: &HashSet<TypeId>) -> bool {
                let mut pass = true;

                $(
                    pass &= $params::criteria(owned_resources);
                )*

                pass
            }

            fn accesses(&self) -> Accesses {
                let mut scheduler_accesses = AccessMap::new();
                let mut system_accesses = AccessMap::new();

                $(
                    $params::accesses(&mut scheduler_accesses, &mut system_accesses);
                )*

                Accesses::new(scheduler_accesses, system_accesses)
            }

            fn needs_system_resource(&self) -> bool {
                $(
                    if $params::select_target() == Target::System {
                        return true;
                    }
                )*

                false
            }
        }
    };
}

// Haskell like
macro_rules! impl_all_sync_system {
    () => {
        impl_sync_system!();
    };

    ($first:ident $(, $rest:ident)*) => {
        impl_sync_system!($first $(, $rest)*);
        impl_all_sync_system!($($rest),*);
    };
}

impl_all_sync_system!(T1, T2, T3, T4, T5, T6, T7, T8, T9);

#[cfg(test)]
mod sync_system_tests {
    use std::{any::TypeId, collections::{HashMap, HashSet}, sync::{Arc, RwLock}};

    use crate::{id::Id, parameters::injections::{shared::Shared, unique::Unique}, scheduler::{accesses::{access::Access, access_map::AccessMap, Accesses}, resources::resource_map::ResourceMap, system_event::SystemResult}, systems::sync_system::{into_sync::IntoSyncSystem, SyncSystem}};

    fn foo(mut channel: Unique<usize>) -> Option<SystemResult> {
        **channel = 1;
        None
    }

    #[test]
    fn can_run() {
        let mut runnable = foo.into_system();

        let scheduler_resource_map = ResourceMap::default();
        
        assert!(scheduler_resource_map.conservatively_insert_auto_default::<usize>().is_ok());
        
        unsafe { runnable.run(
            &scheduler_resource_map, 
            None, 
            Id::from("foo"), 
            Arc::new(RwLock::new(HashMap::default())), 
            None
        ).unwrap() };

        // Safety:
        // No other accesses
        let channel = unsafe { scheduler_resource_map.resolve::<Shared<usize>>().unwrap() };
        assert_eq!(**channel, 1)
    }

    #[test]
    fn can_access() {
        let runnable = foo.into_system();

        assert_eq!(runnable.accesses().scheduler(), Accesses::new(AccessMap::from([(TypeId::of::<usize>(), Access::Unique)]), AccessMap::default()).scheduler());        
    }

    #[test]
    fn can_pass_criteria() {
        let runnable = foo.into_system();
        let binding = TypeId::of::<usize>();
        let owned_resources = HashSet::from([binding]);
        assert!(runnable.criteria(&owned_resources));
    }

    #[test]
    #[should_panic]
    fn can_fail_criteria() {
        let runnable = foo.into_system();
        let owned_resources = HashSet::default();
        assert!(runnable.criteria(&owned_resources));
    }
}