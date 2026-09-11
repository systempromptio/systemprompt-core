//! JSON-Schema sanitisation for provider compatibility.
//!
//! [`SchemaSanitizer`] rewrites a tool/output schema so it only uses constructs
//! the target provider supports: it folds nullable type-arrays into a
//! `nullable` flag, strips unsupported composition keywords
//! (`allOf`/`anyOf`/`oneOf`/`not`, `$ref`, definitions) per the provider's
//! [`ProviderCapabilities`], drops metadata and `x-` extension fields, and
//! recurses through nested schemas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::capabilities::ProviderCapabilities;
use serde_json::Value;

#[derive(Debug, Copy, Clone)]
pub struct SchemaSanitizer {
    capabilities: ProviderCapabilities,
}

impl SchemaSanitizer {
    pub const fn new(capabilities: ProviderCapabilities) -> Self {
        Self { capabilities }
    }

    pub fn sanitize(&self, schema: Value) -> Value {
        let mut sanitized = schema;

        let Some(obj) = sanitized.as_object_mut() else {
            return sanitized;
        };

        Self::normalize_nullable(obj);
        self.remove_unsupported_keywords(obj);
        Self::remove_metadata_fields(obj);
        Self::remove_extension_fields(obj);
        self.convert_const_to_enum(obj);
        if !self.capabilities.features.loose_items {
            Self::split_type_list_into_variants(obj);
            Self::pin_items_to_arrays(obj);
        }
        self.sanitize_nested_schemas(obj);
        if !self.capabilities.features.loose_items {
            Self::type_or_drop_variants(obj);
        }

        sanitized
    }
}

mod json_type;
mod nested;
mod nullable;
mod unsupported;
mod variants;
