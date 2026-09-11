//! JSON-Schema capability matrices and sanitisation, shared by the gateway
//! wire codecs and the agent-flow provider clients.
//!
//! [`ProviderCapabilities`] declares which JSON-Schema constructs each provider
//! accepts; [`SchemaSanitizer`] strips everything outside that set. A wire
//! protocol resolves its matrix via
//! [`crate::services::WireProtocol::schema_capabilities`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod capabilities;
pub mod gemini_invariants;
pub mod sanitizer;

pub use capabilities::{ProviderCapabilities, SchemaComposition, SchemaFeatures};
pub use gemini_invariants::gemini_declaration_violations;
pub use sanitizer::SchemaSanitizer;
