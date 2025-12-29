//! Type-safe extension type definitions.

use std::any::TypeId;

use crate::typed_registry::TypedExtensionRegistry;

pub trait ExtensionType: Default + Send + Sync + 'static {
    const ID: &'static str;
    const NAME: &'static str;
    const VERSION: &'static str;
    const PRIORITY: u32 = 100;

    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}

pub trait ExtensionMeta: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn priority(&self) -> u32;
}

impl<T: ExtensionType> ExtensionMeta for T {
    fn id(&self) -> &'static str {
        T::ID
    }

    fn name(&self) -> &'static str {
        T::NAME
    }

    fn version(&self) -> &'static str {
        T::VERSION
    }

    fn priority(&self) -> u32 {
        T::PRIORITY
    }
}

pub trait Dependencies: ExtensionType {
    type Deps: DependencyList;
}

impl<T: ExtensionType + NoDependencies> Dependencies for T {
    type Deps = ();
}

pub trait NoDependencies {}

pub trait DependencyList: 'static {
    fn validate(registry: &TypedExtensionRegistry) -> Result<(), MissingDependency>;
    fn dependency_ids() -> Vec<&'static str>;
}

impl DependencyList for () {
    fn validate(_: &TypedExtensionRegistry) -> Result<(), MissingDependency> {
        Ok(())
    }

    fn dependency_ids() -> Vec<&'static str> {
        vec![]
    }
}

impl<H: ExtensionType, T: DependencyList> DependencyList for (H, T) {
    fn validate(registry: &TypedExtensionRegistry) -> Result<(), MissingDependency> {
        if !registry.has_type::<H>() {
            return Err(MissingDependency {
                extension_id: H::ID,
                extension_name: H::NAME,
            });
        }
        T::validate(registry)
    }

    fn dependency_ids() -> Vec<&'static str> {
        let mut ids = vec![H::ID];
        ids.extend(T::dependency_ids());
        ids
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MissingDependency {
    pub extension_id: &'static str,
    pub extension_name: &'static str,
}
