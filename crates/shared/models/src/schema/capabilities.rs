//! Per-provider JSON-Schema capability matrices.
//!
//! [`ProviderCapabilities`] declares which JSON-Schema constructs a provider's
//! tool/output schema parser accepts. It is the input to
//! [`super::SchemaSanitizer`], which strips everything a provider does not
//! support. The matrices live here in `shared/models` so both the gateway wire
//! codecs and the agent-flow provider clients resolve the same authority; the
//! wire protocol picks one via
//! [`crate::profile::WireProtocol::schema_capabilities`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaComposition {
    pub allof: bool,
    pub anyof: bool,
    pub oneof: bool,
    pub if_then_else: bool,
    pub not: bool,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "schema feature matrix: each bool is an independent JSON-Schema construct the \
              provider does or does not accept, not state"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaFeatures {
    pub references: bool,
    pub definitions: bool,
    pub additional_properties: bool,
    pub const_values: bool,
    /// `exclusiveMinimum` / `exclusiveMaximum` numeric bounds. Gemini's
    /// OpenAPI-subset parser rejects these; Anthropic and `OpenAI` accept them.
    pub exclusive_bounds: bool,
    /// `propertyNames` / `patternProperties` object constraints. Rejected by
    /// Gemini's parser; accepted by Anthropic and `OpenAI`.
    pub property_names: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub composition: SchemaComposition,
    pub features: SchemaFeatures,
}

impl ProviderCapabilities {
    pub const fn anthropic() -> Self {
        Self {
            composition: SchemaComposition {
                allof: true,
                anyof: true,
                oneof: true,
                if_then_else: true,
                not: true,
            },
            features: SchemaFeatures {
                references: true,
                definitions: true,
                additional_properties: true,
                const_values: true,
                exclusive_bounds: true,
                property_names: true,
            },
        }
    }

    pub const fn openai() -> Self {
        Self {
            composition: SchemaComposition {
                allof: true,
                anyof: true,
                oneof: true,
                if_then_else: false,
                not: false,
            },
            features: SchemaFeatures {
                references: true,
                definitions: true,
                additional_properties: true,
                const_values: true,
                exclusive_bounds: true,
                property_names: true,
            },
        }
    }

    pub const fn gemini() -> Self {
        Self {
            composition: SchemaComposition {
                allof: false,
                anyof: true,
                oneof: false,
                if_then_else: false,
                not: false,
            },
            features: SchemaFeatures {
                references: false,
                definitions: false,
                additional_properties: false,
                const_values: false,
                exclusive_bounds: false,
                property_names: false,
            },
        }
    }

    pub fn requires_transformation(&self, schema: &Value) -> bool {
        if let Some(obj) = schema.as_object() {
            if obj.contains_key("allOf") && !self.composition.allof {
                return true;
            }
            if obj.contains_key("anyOf") && !self.composition.anyof {
                return true;
            }
            if obj.contains_key("oneOf") && !self.composition.oneof {
                return true;
            }
            if obj.contains_key("if") && !self.composition.if_then_else {
                return true;
            }
            if obj.contains_key("$ref") && !self.features.references {
                return true;
            }
            if (obj.contains_key("definitions") || obj.contains_key("$defs"))
                && !self.features.definitions
            {
                return true;
            }
            if obj.contains_key("not") && !self.composition.not {
                return true;
            }
        }
        false
    }
}
