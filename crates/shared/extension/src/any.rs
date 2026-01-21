use std::any::Any;
use std::fmt::Debug;

#[cfg(feature = "web")]
use crate::typed::ApiExtensionTypedDyn;
use crate::typed::{
    ConfigExtensionTyped, JobExtensionTyped, ProviderExtensionTyped, SchemaExtensionTyped,
};
use crate::types::ExtensionType;

pub trait AnyExtension: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn priority(&self) -> u32;

    fn as_schema(&self) -> Option<&dyn SchemaExtensionTyped> {
        None
    }
    #[cfg(feature = "web")]
    fn as_api(&self) -> Option<&dyn ApiExtensionTypedDyn> {
        None
    }
    fn as_config(&self) -> Option<&dyn ConfigExtensionTyped> {
        None
    }
    fn as_job(&self) -> Option<&dyn JobExtensionTyped> {
        None
    }
    fn as_provider(&self) -> Option<&dyn ProviderExtensionTyped> {
        None
    }

    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct ExtensionWrapper<T: Debug> {
    inner: T,
}

impl<T: ExtensionType + Debug> ExtensionWrapper<T> {
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

#[derive(Debug)]
pub struct SchemaExtensionWrapper<T: Debug> {
    inner: T,
}

impl<T: ExtensionType + SchemaExtensionTyped + Debug> SchemaExtensionWrapper<T> {
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

#[cfg(feature = "web")]
#[derive(Debug)]
pub struct ApiExtensionWrapper<T: Debug> {
    inner: T,
}

#[cfg(feature = "web")]
impl<T: ExtensionType + ApiExtensionTypedDyn + Debug> ApiExtensionWrapper<T> {
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "web")]
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
