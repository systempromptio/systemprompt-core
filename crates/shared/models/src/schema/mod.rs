//! JSON-Schema capability matrices and sanitisation, shared by the gateway
//! wire codecs and the agent-flow provider clients.
//!
//! [`ProviderCapabilities`] declares which JSON-Schema constructs each provider
//! accepts; [`SchemaSanitizer`] strips everything outside that set. A wire
//! protocol resolves its matrix via
//! [`crate::profile::WireProtocol::schema_capabilities`].

pub mod capabilities;
pub mod sanitizer;

pub use capabilities::{ProviderCapabilities, SchemaComposition, SchemaFeatures};
pub use sanitizer::SchemaSanitizer;
