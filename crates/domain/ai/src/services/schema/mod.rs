//! Schema utilities — provider capability matrices, transformer to coerce
//! tool input schemas to provider-specific shapes, and a sanitiser pass.

pub mod analyzer;
pub mod capabilities;
pub mod mapper;
pub mod sanitizer;
pub mod transformer;

pub use analyzer::DiscriminatedUnion;
pub use capabilities::ProviderCapabilities;
pub use mapper::ToolNameMapper;
pub use sanitizer::SchemaSanitizer;
pub use transformer::{SchemaTransformer, TransformedTool};
