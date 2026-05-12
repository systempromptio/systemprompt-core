//! # systemprompt-templates
//!
//! Template registry, loading, and rendering for systemprompt.io. The crate
//! exposes:
//!
//! - [`TemplateRegistry`] — Handlebars-backed engine with multi-source template
//!   discovery, partial registration, and stat reporting. Registers a `json`
//!   helper for emitting values inside JSON contexts (e.g. JSON-LD `<script>`
//!   blocks): `{{{json field}}}` serialises via `serde_json::to_string`, which
//!   correctly escapes backslashes, newlines, and control characters that
//!   Handlebars' default HTML escaping leaves intact.
//! - [`TemplateRegistryBuilder`] — fluent builder that wires together loaders,
//!   providers, and partial sources.
//! - [`CoreTemplateProvider`] — filesystem provider scanning a directory for
//!   `.html` templates plus an optional `templates.yaml` manifest.
//! - [`EmbeddedDefaultsProvider`] — bundles the in-tree `defaults/` templates
//!   so consumers get a usable engine without touching disk.
//! - Re-exports of the `systemprompt-template-provider` traits the engine
//!   composes against.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | _none_  | n/a     | The crate exposes a single feature surface; all modules are compiled unconditionally. The `[package.metadata.docs.rs] all-features = true` setting is retained so future feature additions automatically appear in published docs. |
//!
//! ## Layering
//!
//! `systemprompt-templates` is a **domain** crate whose only systemprompt
//! dependency is the `systemprompt-template-provider` shared crate.

mod builder;
mod core_provider;
mod embedded_defaults;
pub mod error;
mod registry;

pub use builder::TemplateRegistryBuilder;
pub use core_provider::CoreTemplateProvider;
pub use embedded_defaults::EmbeddedDefaultsProvider;
pub use error::{TemplateError, TemplateResult};
pub use registry::{RegistryStats, TemplateRegistry};

pub use systemprompt_template_provider::{
    ComponentContext, ComponentRenderer, DynComponentRenderer, DynPageDataProvider,
    DynTemplateDataExtender, DynTemplateLoader, DynTemplateProvider, EmbeddedLoader, ExtendedData,
    ExtenderContext, FileSystemLoader, PageContext, PageDataProvider, RenderedComponent,
    TemplateDataExtender, TemplateDefinition, TemplateLoader, TemplateProvider, TemplateSource,
};
