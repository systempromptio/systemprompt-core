//! Type-level identifiers, dependency lists, and the [`ExtensionMeta`]
//! supertrait used by typed extension declarations.

use std::any::TypeId;

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
    fn dependency_ids() -> Vec<&'static str>;
}

impl DependencyList for () {
    fn dependency_ids() -> Vec<&'static str> {
        vec![]
    }
}

impl<H: ExtensionType, T: DependencyList> DependencyList for (H, T) {
    fn dependency_ids() -> Vec<&'static str> {
        let mut ids = vec![H::ID];
        ids.extend(T::dependency_ids());
        ids
    }
}
