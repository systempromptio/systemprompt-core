//! Document templates: the base documents new files are created from.
//!
//! Templates come from two sources. Compile-time
//! [`DocumentTemplateProvider`] registrations ship embedded templates (core
//! registers [`CoreTemplateProvider`] at priority 100 with the blank text,
//! Markdown and HTML documents), and the services tree carries a YAML
//! catalog under `documents/templates/*.yaml` whose entries point at files
//! beside them. [`TemplateRegistry`] merges both once at a composition root;
//! when two sources publish the same template id the higher priority wins.
//! Blanks whose bytes are generated (the office formats) are added through
//! [`office_blanks`] and [`TemplateRegistry::with_generated`].
//! Extensions ship branded templates either way, as a provider or as
//! catalog entries in their services bundle.
//!
//! [`DocumentTemplateProvider`]: systemprompt_provider_contracts::documents::DocumentTemplateProvider
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
mod core;
mod registry;

pub use self::catalog::{
    CATALOG_PRIORITY, CatalogTemplate, TemplateCatalogError, catalog_dir, load_catalog,
};
pub use self::core::{
    CORE_TEMPLATE_PRIORITY, CoreTemplateProvider, GeneratedBlank, generated_blank, office_blanks,
};
pub use self::registry::{InstantiatedTemplate, TemplateError, TemplateRegistry};
