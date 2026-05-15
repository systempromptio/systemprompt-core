//! Template provider traits for systemprompt.io's AI-governance template
//! registry.
//!
//! This crate is the config-as-code foundation for rendering pages, components,
//! and partials. It exposes a small set of traits — [`TemplateProvider`],
//! [`TemplateLoader`], [`TemplateDataExtender`], [`ComponentRenderer`],
//! [`PageDataProvider`], and [`PagePrerenderer`] — together with two ready-made
//! loaders ([`EmbeddedLoader`] and, behind the `tokio` feature,
//! `FileSystemLoader`).
//!
//! Concrete provider implementations live in `systemprompt-templates`;
//! consumers depend on this crate only when they need to **plug in** to that
//! pipeline (custom loaders, custom extenders, custom prerenderers).
//!
//! # Feature flags
//!
//! | Feature | Default | Adds |
//! |---------|---------|------|
//! | `tokio` | no | `FileSystemLoader`, a `tokio::fs`-backed [`TemplateLoader`] with base-path sandboxing |
//!
//! All public items are documented and `docs.rs` is built with
//! `--all-features`.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use systemprompt_template_provider::{DynTemplateLoader, EmbeddedLoader};
//!
//! let loader: DynTemplateLoader = Arc::new(EmbeddedLoader);
//! ```

pub mod traits;

pub use traits::{
    ComponentContext, ComponentRenderer, EmbeddedLoader, ExtendedData, ExtenderContext,
    PageContext, PageDataProvider, PartialSource, PartialTemplate, RenderedComponent,
    TemplateDataExtender, TemplateDefinition, TemplateLoader, TemplateLoaderError,
    TemplateLoaderResult, TemplateProvider, TemplateSource,
};

pub use systemprompt_provider_contracts::{
    PagePrepareContext, PagePrerenderer, PageRenderSpec, ProviderError, ProviderResult,
};

#[cfg(feature = "tokio")]
pub use traits::FileSystemLoader;

pub type DynTemplateProvider = std::sync::Arc<dyn TemplateProvider>;

pub type DynTemplateLoader = std::sync::Arc<dyn TemplateLoader>;

pub type DynTemplateDataExtender = std::sync::Arc<dyn TemplateDataExtender>;

pub type DynComponentRenderer = std::sync::Arc<dyn ComponentRenderer>;

pub type DynPageDataProvider = std::sync::Arc<dyn PageDataProvider>;

pub type DynPagePrerenderer = std::sync::Arc<dyn PagePrerenderer>;
