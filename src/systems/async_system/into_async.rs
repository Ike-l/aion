use crate::{parameters::InjectionParam, systems::{async_system::AsyncSystem, FunctionSystem}};

pub trait IntoAsyncSystem<Input> {
    type System: AsyncSystem;

    fn into_system(self) -> Self::System;
}

macro_rules! impl_into_async_system {
    (
        $($params:ident),*
    ) => {
        impl<F, Fut, $($params: InjectionParam),*> IntoAsyncSystem<($($params,)*)> for F
        where
            Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
            F: Send + Sync,
            // Since cant do &'a mut F since impl Future needs ownership means cant tie lifetime of parameters to lifetime of caller hence InjectionParam with a lifetime won't work in async functions (can clone or arc clone?)
            for<'b> F: 
                FnMut($($params),*) -> Fut +
                FnMut($(<$params as InjectionParam>::Item<'b>),*) -> Fut,
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
// impl<F, Fut, $($params: InjectionParam),*> IntoAsyncSystem<($($params,)*)> for F
// where
//     Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
//     F: Send + Sync,
//     // Since cant do &'a mut F since impl Future needs ownership means cant tie lifetime of parameters to lifetime of caller hence InjectionParam with a lifetime won't work in async functions (can clone or arc clone?)
//     for<'b> F: 
//         FnMut($($params),*) -> Fut +
//         FnMut($(<$params as InjectionParam>::Item<'b>),*) -> Fut,
// {
//     type System = FunctionSystem<($($params,)*), Self>;

//     fn into_system(self) -> Self::System {
//         FunctionSystem {
//             f: self,
//             marker: Default::default(),
//         }
//     }
// }

macro_rules! impl_all_into_async_system {
    () => {
        impl_into_async_system!();
    };

    ($first:ident $(, $rest:ident)*) => {
        impl_into_async_system!($first $(, $rest)*);
        impl_all_into_async_system!($($rest),*);
    };
}

impl_all_into_async_system!(T1, T2, T3, T4, T5, T6, T7, T8, T9);