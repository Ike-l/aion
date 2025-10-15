use crate::{injection::injection_trait::Injection, system::{sync_system::SyncSystem, system_result::SystemResult, FunctionSystem}};

pub trait IntoSyncSystem<Input> {
    type System: SyncSystem;

    fn into_system(self) -> Self::System;
}

macro_rules! impl_into_sync_system {
    (
        $($params:ident),*
    ) => {
        impl<F, $($params: Injection),*> IntoSyncSystem<($($params,)*)> for F
            where
                F: Send + Sync,
                for<'a, 'b> &'a mut F:
                    FnMut($($params),*) -> Option<SystemResult> +
                    FnMut($(<$params as Injection>::Item<'b>),*) -> Option<SystemResult> 
        {
            type System = FunctionSystem<($($params,)*), Self>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    f: self,
                    marker: Default::default(),
                }
            }
        }
    };
}

// Haskell like
macro_rules! impl_all_into_sync_system {
    () => {
        impl_into_sync_system!();
    };

    ($first:ident $(, $rest:ident)*) => {
        impl_into_sync_system!($first $(, $rest)*);
        impl_all_into_sync_system!($($rest),*);
    };
}


impl_all_into_sync_system!(T1, T2, T3, T4, T5, T6, T7, T8, T9);