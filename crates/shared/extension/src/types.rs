//! Type-level identifiers, dependency lists, and the [`ExtensionMeta`]
//! supertrait used by typed extension declarations.

use std::any::TypeId;

use crate::typed_registry::TypedExtensionRegistry;

/// Static-type marker for an extension. Implementors expose their `id`,
/// `name`, `version`, and registration `priority` as associated constants
/// so a typed registry can identify them without instantiation.
pub trait ExtensionType: Default + Send + Sync + 'static {
    /// Stable extension identifier (kebab-case).
    const ID: &'static str;
    /// Human-readable extension name.
    const NAME: &'static str;
    /// Semver-style version string.
    const VERSION: &'static str;
    /// Registration priority (lower runs first).
    const PRIORITY: u32 = 100;

    /// Returns the `TypeId` of this extension type.
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}

/// Object-safe surface that exposes an extension's metadata for runtime
/// lookup.
///
/// Auto-implemented for every [`ExtensionType`].
pub trait ExtensionMeta: Send + Sync + 'static {
    /// Stable extension identifier.
    fn id(&self) -> &'static str;
    /// Human-readable extension name.
    fn name(&self) -> &'static str;
    /// Semver-style version string.
    fn version(&self) -> &'static str;
    /// Registration priority (lower runs first).
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

/// Declares the dependency list of an extension at the type level.
///
/// Provides a blanket impl for any [`ExtensionType`] that also implements
/// [`NoDependencies`].
pub trait Dependencies: ExtensionType {
    /// Heterogeneous list of dependency types.
    type Deps: DependencyList;
}

impl<T: ExtensionType + NoDependencies> Dependencies for T {
    type Deps = ();
}

/// Marker trait for extensions with no dependencies. Implement this to
/// opt into the blanket [`Dependencies`] impl.
pub trait NoDependencies {}

/// Operations supported by a heterogeneous list of dependency types.
pub trait DependencyList: 'static {
    /// Verifies every type in the list is registered in `registry`.
    fn validate(registry: &TypedExtensionRegistry) -> Result<(), MissingDependency>;
    /// Returns the IDs of every type in the list.
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

/// Failure raised when a typed dependency cannot be resolved against the
/// registry.
#[derive(Debug, Clone, Copy)]
pub struct MissingDependency {
    /// ID of the missing dependency.
    pub extension_id: &'static str,
    /// Human-readable name of the missing dependency.
    pub extension_name: &'static str,
}
