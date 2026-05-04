//! Type-erased wrappers used by [`crate::TypedExtensionRegistry`].
//!
//! Each variant — [`ExtensionWrapper`], [`SchemaExtensionWrapper`],
//! [`ApiExtensionWrapper`] — boxes a concrete extension type and exposes a
//! uniform [`AnyExtension`] surface so the registry can store
//! heterogeneous registrations in a single `Vec`.

use std::any::Any;
use std::fmt::Debug;

use crate::typed::{
    ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped, ProviderExtensionTyped,
    SchemaExtensionTyped,
};
use crate::types::ExtensionType;

/// Type-erased view of any registered extension. Yields concrete typed
/// references via the `as_*` downcast methods.
pub trait AnyExtension: Send + Sync + 'static {
    /// Returns the extension's stable identifier.
    fn id(&self) -> &'static str;
    /// Returns the extension's human-readable name.
    fn name(&self) -> &'static str;
    /// Returns the extension's semver-style version string.
    fn version(&self) -> &'static str;
    /// Returns the registration priority (lower runs first).
    fn priority(&self) -> u32;

    /// Returns this extension as a [`SchemaExtensionTyped`], if it
    /// contributes a schema.
    fn as_schema(&self) -> Option<&dyn SchemaExtensionTyped> {
        None
    }
    /// Returns this extension as an [`ApiExtensionTypedDyn`], if it
    /// contributes a router.
    fn as_api(&self) -> Option<&dyn ApiExtensionTypedDyn> {
        None
    }
    /// Returns this extension as a [`ConfigExtensionTyped`], if it
    /// declares a config namespace.
    fn as_config(&self) -> Option<&dyn ConfigExtensionTyped> {
        None
    }
    /// Returns this extension as a [`JobExtensionTyped`], if it
    /// contributes scheduled jobs.
    fn as_job(&self) -> Option<&dyn JobExtensionTyped> {
        None
    }
    /// Returns this extension as a [`ProviderExtensionTyped`], if it
    /// contributes provider implementations.
    fn as_provider(&self) -> Option<&dyn ProviderExtensionTyped> {
        None
    }

    /// Returns the wrapped value as `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
    /// Returns the Rust type name of the wrapped extension.
    fn type_name(&self) -> &'static str;
}

/// Wrapper that exposes an [`ExtensionType`] as a plain [`AnyExtension`]
/// without any of the typed sub-trait facets.
#[derive(Debug)]
pub struct ExtensionWrapper<T: Debug> {
    inner: T,
}

impl<T: ExtensionType + Debug> ExtensionWrapper<T> {
    /// Constructs a new wrapper around `inner`.
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ExtensionType + Debug + 'static> AnyExtension for ExtensionWrapper<T> {
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

    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Wrapper that additionally exposes a [`SchemaExtensionTyped`] facet.
#[derive(Debug)]
pub struct SchemaExtensionWrapper<T: Debug> {
    inner: T,
}

impl<T: ExtensionType + SchemaExtensionTyped + Debug> SchemaExtensionWrapper<T> {
    /// Constructs a new schema wrapper around `inner`.
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ExtensionType + SchemaExtensionTyped + Debug + 'static> AnyExtension
    for SchemaExtensionWrapper<T>
{
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

    fn as_schema(&self) -> Option<&dyn SchemaExtensionTyped> {
        Some(&self.inner)
    }

    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Wrapper that additionally exposes an [`ApiExtensionTypedDyn`] facet.
#[derive(Debug)]
pub struct ApiExtensionWrapper<T: Debug> {
    inner: T,
}

impl<T: ExtensionType + ApiExtensionTypedDyn + Debug> ApiExtensionWrapper<T> {
    /// Constructs a new API wrapper around `inner`.
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: ExtensionType + ApiExtensionTypedDyn + Debug + 'static> AnyExtension
    for ApiExtensionWrapper<T>
{
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

    fn as_api(&self) -> Option<&dyn ApiExtensionTypedDyn> {
        Some(&self.inner)
    }

    fn as_any(&self) -> &dyn Any {
        &self.inner
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}
