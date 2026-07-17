//! Schema utilities — transformer to coerce tool input schemas to
//! provider-specific shapes, and a discriminated-union analyzer.
//!
//! The provider capability matrices ([`ProviderCapabilities`]) and the
//! sanitiser ([`SchemaSanitizer`]) live in `systemprompt_models::schema` so the
//! gateway wire codecs and the agent-flow provider clients share one authority;
//! they are re-exported here for the agent-side call sites.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analyzer;
pub mod mapper;
pub mod transformer;

pub use analyzer::DiscriminatedUnion;
pub use mapper::ToolNameMapper;
pub use systemprompt_models::schema::{
    ProviderCapabilities, SchemaComposition, SchemaFeatures, SchemaSanitizer,
};
pub use transformer::{SchemaTransformer, TransformedTool};
