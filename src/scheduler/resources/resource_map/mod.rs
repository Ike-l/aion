use std::{any::{Any, TypeId}, collections::hash_map::Entry};

use anyhow::Context;

use crate::{parameters::InjectionParam, scheduler::resources::{resource_wrapper::ResourceWrapper, Resource}};

pub mod inner_resource_map;

#[derive(Debug, Default)]
pub struct ResourceMap {
    in_use: parking_lot::RwLock<()>,
    resources: inner_resource_map::InnerResourceMap,
}

impl ResourceMap {
    pub fn keys(&self) -> impl Iterator<Item = &TypeId> {
        self.resources.get_map(&self.in_use).0.keys()
    }

    pub fn conservatively_insert<T: 'static>(&self, type_id: TypeId, resource: T) -> anyhow::Result<()> {
        let (map, _writing) = self.resources.get_map_mut(&self.in_use);

        match map.entry(type_id) {
            Entry::Occupied(_) => anyhow::bail!("Resource of this type already exists"),
            Entry::Vacant(entry) => {
                let v: Box<dyn Any> = Box::new(resource);
                entry.insert(ResourceWrapper::new(v));
                return Ok(());
            }
        }
    }

    pub fn conservatively_insert_auto<T: 'static>(&self, resource: T) -> anyhow::Result<()> {
        self.conservatively_insert(TypeId::of::<T>(), resource).context("From conservatively_insert_auto")
    }

    pub fn conservatively_insert_auto_default<T: 'static + Default>(&self) -> anyhow::Result<()> {
        self.conservatively_insert_auto(T::default()).context("From conservatively_insert_auto_default")
    }

    pub fn conservatively_merge(&self, other: Self) -> anyhow::Result<()> {
        let (map, _writing) = self.resources.get_map_mut(&self.in_use);

        let mut errs = Vec::new();
        for (other_ty, other_res) in other.resources.consume() {
            match map.entry(other_ty) {
                Entry::Occupied(_) => errs.push(format!("Existing resource of type: {:?}", other_ty)),
                Entry::Vacant(entry) => { entry.insert(other_res); },
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            let combined = errs.join("\n");
            Err(anyhow::Error::msg(combined))
        }
    }

    pub fn resolve<T: InjectionParam>(&self) -> Option<T::Item<'_>> {
        unsafe { T::try_retrieve(&self) }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        let (map, _reading) = self.resources.get_map(&self.in_use);
        unsafe {
            map
                .get(&TypeId::of::<T>())
                .map(|cell| & *cell.get())
                .and_then(|boxed| boxed.downcast_ref::<T>())
        }
    }

    pub fn get_mut<T: 'static>(&self) -> Option<&mut T> {
        let (map, _reading) = self.resources.get_map(&self.in_use);
        unsafe {
            map
                .get(&TypeId::of::<T>())
                .map(|cell| &mut *cell.get())
                .and_then(|boxed| boxed.downcast_mut::<T>())
        }
    }

    pub fn insert<T: 'static>(&mut self, type_id: TypeId, resource: T) -> Option<Resource> {
        let resource: Box<dyn Any> = Box::new(resource);
        self.resources.get_map_mut(&self.in_use).0.insert(type_id, ResourceWrapper::new(resource))
    }

    pub fn insert_auto<T: 'static>(&mut self, resource: T) -> Option<Resource> {
        self.insert(TypeId::of::<T>(), resource)
    }

    pub fn insert_auto_default<T: 'static + Default>(&mut self) -> Option<Resource> {
        self.insert_auto(T::default())
    }
}