//! The catalog builder and canned catalogs (`CatalogFixture`,
//! `one_platform_provider`, `opencode_go`, `vertex_regions`).
//!
//! They live in `systemprompt-catalog-fixtures`, which depends only on the
//! manifest, so a test crate that needs a `ResolvedCatalog` and nothing
//! DB-backed can take it without this crate's runtime dependencies.

pub use systemprompt_catalog_fixtures::*;
