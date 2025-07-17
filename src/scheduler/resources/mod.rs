pub mod resource_map;
pub mod resource_wrapper;
pub mod system_resource;
pub mod new_resources;

pub type Resource = resource_wrapper::ResourceWrapper<Box<dyn std::any::Any>>;