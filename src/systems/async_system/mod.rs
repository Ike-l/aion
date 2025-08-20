use std::{any::TypeId, collections::{HashMap, HashSet}, pin::Pin, sync::{Arc, RwLock}};

use crate::{id::Id, parameters::{InjectionParam, Target}, scheduler::{accesses::{access_map::AccessMap, Accesses}, resources::{resource_map::ResourceMap, system_resource::{system_resource_ptr::SystemResourcePtr, SystemResource}}}, systems::FunctionSystem};

pub mod into_async;
pub mod waker;

pub trait AsyncSystem: Send + Sync {
    // can ?trivially make the return future `+ Send`
    /// Safety:
    /// Ensure no concurrent mutable accesses via `fn accesses`
    unsafe fn run<'a>(
        &'a mut self,
        scheduler_resource_map: &'a ResourceMap,
        running_system_resource_map: Option<&'a SystemResourcePtr>,
        running_system_id: Id,
        ids: Arc<RwLock<HashMap<u64, String>>>,
        system_resource_maps: Option<&'a HashMap<Id, Arc<SystemResource>>>
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>;

    /// Does the scheduler have the resources the SystemParam needs?
    fn criteria(&self, owned_resources: &HashSet<TypeId>) -> bool;
    fn accesses(&self) -> Accesses;
    fn needs_system_resource(&self) -> bool;
}

macro_rules! impl_async_system {
    (
        $($params:ident),*
    ) => {
        #[allow(clippy::too_many_arguments)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        impl<F, Fut, $($params: InjectionParam),*> AsyncSystem for FunctionSystem<($($params,)*), F>
        where
            Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
            F: Send + Sync,
            for<'b> F: 
                FnMut($($params),*) -> Fut +
                FnMut($(<$params as InjectionParam>::Item<'b>),*) -> Fut,
        {
            unsafe fn run<'a>(
                &'a mut self,
                scheduler_resource_map: &'a ResourceMap,
                system_resource_map: Option<&'a SystemResourcePtr>,
                system_id: Id,
                id_map: Arc<RwLock<HashMap<u64, String>>>,
                system_resource_maps: Option<&'a HashMap<Id, Arc<SystemResource>>>,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>> {
                Box::pin(async move {
                    $(
                        let $params = unsafe { $params::retrieve(
                            scheduler_resource_map, 
                            system_resource_map,
                            system_id.clone(), 
                            id_map.read().unwrap(),
                            system_resource_maps, 
                        )? };
                    )*

                    drop(id_map);
                    
                    (self.f)($($params),*).await
                })                
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

macro_rules! impl_all_async_system {
    () => {
        impl_async_system!();
    };

    ($first:ident $(, $rest:ident)*) => {
        impl_async_system!($first $(, $rest)*);
        impl_all_async_system!($($rest),*);
    };
}

impl_all_async_system!(T1, T2, T3, T4, T5, T6, T7, T8, T9);

#[cfg(test)]
mod async_system_tests {
    use std::{any::TypeId, collections::{HashMap, HashSet}, sync::{Arc, RwLock}, task::{Context, Poll, Waker}, time::Duration};

    use anyhow::Ok;

    use crate::{id::Id, parameters::injections::{arc_mutex::ArcMutex, shared::Shared, take::Take}, scheduler::{accesses::{access::Access, access_map::AccessMap, Accesses}, resources::resource_map::ResourceMap}, systems::async_system::{into_async::IntoAsyncSystem, waker::DummyWaker, AsyncSystem}};

    async fn foo(channel: ArcMutex<usize>) -> anyhow::Result<()> {
        *pollster::block_on(channel.lock()) = 1;
        Ok(())
    }

    async fn bar(duration: Take<f32>) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_secs_f32(*duration)).await;
        Ok(())
    }
 
    #[test]
    fn can_run() {
        let mut runnable = foo.into_system();

        let scheduler_resource_map = ResourceMap::default();
        
        assert!(scheduler_resource_map.conservatively_insert_auto_default::<Arc<tokio::sync::Mutex<usize>>>().is_ok());
        
        pollster::block_on(unsafe { runnable.run(
            &scheduler_resource_map, 
            None, 
            Id::from("foo"), 
            Arc::new(RwLock::new(HashMap::default())), 
            None
        ) }).unwrap();

        // Safety:
        // No other accesses
        let channel = unsafe { scheduler_resource_map.resolve::<Shared<Arc<tokio::sync::Mutex<usize>>>>().unwrap() };
        assert_eq!(*channel.blocking_lock(), 1)
    }

    #[test]
    fn can_block() {
        let mut runnable = bar.into_system();

        let scheduler_resource_map = ResourceMap::default();

        const DURATION: f32 = 0.1;

        assert!(scheduler_resource_map.conservatively_insert_auto::<f32>(DURATION).is_ok());

        let waker = Waker::from(Arc::new(DummyWaker));
        let mut cx = Context::from_waker(&waker);

        let rt = tokio::runtime::Runtime::new().unwrap();

        // done like this to mimic how it would be done in the scheduler
        rt.block_on(async move {
            let mut fut = unsafe { runnable.run(
                &scheduler_resource_map, 
                None, 
                Id::from("foo"), 
                Arc::new(RwLock::new(HashMap::default())), 
                None
            ) };
    
            assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
            tokio::time::sleep(Duration::from_secs_f32(DURATION)).await;
            assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Ready(_)));
        });
    }

    #[test]
    fn can_access() {
        let runnable = foo.into_system();

        assert_eq!(runnable.accesses().scheduler(), Accesses::new(AccessMap::from([(TypeId::of::<Arc<tokio::sync::Mutex<usize>>>(), Access::Shared)]), AccessMap::default()).scheduler());        
    }

    #[test]
    fn can_pass_criteria() {
        let runnable = foo.into_system();
        let binding = TypeId::of::<Arc<tokio::sync::Mutex<usize>>>();
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