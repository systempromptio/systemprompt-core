//! Template registry and management for `SystemPrompt`.
//!
//! This crate provides the core template system including:
//! - [`TemplateRegistry`]: Central registry for managing templates, loaders, and extenders
//! - [`CoreTemplateProvider`]: Default provider that discovers templates from the filesystem
//! - [`TemplateError`]: Error types for template operations
//!
//! # Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use systemprompt_templates::{TemplateRegistryBuilder, CoreTemplateProvider, FileSystemLoader};
//!
//! async fn example() -> Result<(), systemprompt_templates::TemplateError> {
//!     let provider = CoreTemplateProvider::discover_from("./templates").await.unwrap();
//!     let loader = FileSystemLoader::new(vec!["./templates".into()]);
//!     let registry = TemplateRegistryBuilder::new()
//!         .with_provider(Arc::new(provider))
//!         .with_loader(Arc::new(loader))
//!         .build_and_init()
//!         .await?;
//!     Ok(())
//! }
//! ```

mod core_provider;
mod error;
mod registry;

pub use core_provider::CoreTemplateProvider;
pub use error::TemplateError;
pub use registry::{RegistryStats, TemplateRegistry, TemplateRegistryBuilder};

pub use systemprompt_template_provider::{
    ComponentContext, ComponentRenderer, DynComponentRenderer, DynPageDataProvider,
    DynTemplateDataExtender, DynTemplateLoader, DynTemplateProvider, EmbeddedLoader, ExtendedData,
    ExtenderContext, FileSystemLoader, PageContext, PageDataProvider, RenderedComponent,
    TemplateDataExtender, TemplateDefinition, TemplateLoader, TemplateProvider, TemplateSource,
};
