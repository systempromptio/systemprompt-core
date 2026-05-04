//! Trait surface for the template provider pipeline.
//!
//! Re-exports the [`TemplateLoader`] family defined locally and the
//! higher-level provider traits that live in `systemprompt-provider-contracts`.
//! Splitting the loader trait out keeps the `tokio`-gated [`FileSystemLoader`]
//! in the same crate as its trait, while the cross-crate provider contracts
//! stay filesystem-agnostic.

mod error;
mod loader;

pub use error::{Result as TemplateLoaderResult, TemplateLoaderError};
#[cfg(feature = "tokio")]
pub use loader::FileSystemLoader;
pub use loader::{EmbeddedLoader, TemplateLoader};
pub use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ExtendedData, ExtenderContext, PageContext,
    PageDataProvider, PartialSource, PartialTemplate, RenderedComponent, TemplateDataExtender,
    TemplateDefinition, TemplateProvider, TemplateSource,
};
