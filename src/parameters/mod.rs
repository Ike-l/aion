pub mod injections;

use std::{any::{type_name, TypeId}, collections::{HashMap, HashSet}, sync::{Arc, RwLockReadGuard}};

use crate::{id::system_id::SystemId, scheduler::{accesses::{access::Access, access_map::AccessMap}, resources::{resource_map::ResourceMap, system_resource::{system_resource_ptr::SystemResourcePtr, SystemResource}}}};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Target {
    Scheduler,
    System,
}

pub trait InjectionParam {
    type Item<'new>;

    fn failed_message() -> String;
    fn resolve_accesses(accesses: &mut AccessMap);
    
    /// Does the scheduler have the resources the SystemParam needs?
    /// Default implementation compares `Accesses`
    fn criteria(owned_resources: &HashSet<&TypeId>) -> bool {
        match Self::select_target() {
            Target::Scheduler => {
                let mut accesses = AccessMap::new();
                Self::resolve_accesses(&mut accesses);
                accesses.accesses.iter().all(|(key, _)| owned_resources.contains(key))
            },
            Target::System => true 
        }
    }

    unsafe fn try_retrieve<'a>(resource_map: &'a ResourceMap) -> Option<Self::Item<'a>>;
    
    fn select_target() -> Target {
        Target::Scheduler
    }

    /// Overriding means you opt out of "nesting" ability, `InjectionParam`s expect to be entered by `try_retrieve` with the correct resource map. 
    /// <br>Hence, for example `Local` will not work because `Local` calls try_retrieve of the `InjectionParam`
    /// <br>SystemResourceMap is Some when Accesses has SystemAccesses
    /// <br>SystemResourceMaps is None when calling for a background thread (so `tick` iterations aren't blocked by a background thread that may never exit)
    fn retrieve<'a>(
        scheduler_resource_map: &'a ResourceMap,
        system_resource_map: Option<&'a SystemResourcePtr>, 
        // Do i want to pass these to try_retrieve?
        _system_id: SystemId, 
        _id_map: RwLockReadGuard<HashSet<String>>,
        _system_resource_maps: Option<&'a HashMap<
            SystemId, 
            Arc<SystemResource>
        >>,
        ) -> anyhow::Result<Self::Item<'a>> { 
        let map = match Self::select_target() {
            Target::Scheduler => scheduler_resource_map,
            Target::System => system_resource_map.ok_or_else(|| anyhow::anyhow!("Missing system_resource_map when Target::System"))?,
        };

        unsafe { Self::try_retrieve(map).ok_or_else(|| anyhow::anyhow!(Self::failed_message())) }
    }

    fn accesses(scheduler_accesses: &mut AccessMap, system_accesses: &mut AccessMap) {
        let map = match Self::select_target() {
            Target::Scheduler => scheduler_accesses,
            Target::System => system_accesses
        };

        Self::resolve_accesses(map);
    }

    fn access<T: 'static>(accesses: &mut AccessMap, access: Access) {
        match access {
            Access::Shared => {
                assert_eq!(
                    *accesses.accesses.entry(TypeId::of::<T>()).or_insert(Access::Shared), Access::Shared,
                    "{}", Self::conflict_message(type_name::<T>()),
                )
            },
            Access::Unique => {
                assert!(
                    accesses.accesses.insert(TypeId::of::<T>(), Access::Unique).is_none(),
                    "{}", Self::conflict_message(type_name::<T>())
                );  
            }
        }
    }

    fn conflict_message(type_name: &'static str) -> String {
        format!("conflicting access in system; from {type_name}")
    }

    fn try_typed_retrieve<T: 'static>(resources: &ResourceMap) -> Option<&T> {
        resources.get::<T>()
    }

    fn try_typed_mut_retrieve<T: 'static>(resources: &ResourceMap) -> Option<&mut T> {
        resources.get_mut::<T>()
    }
}