use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaComposition {
    pub allof: bool,
    pub anyof: bool,
    pub oneof: bool,
    pub if_then_else: bool,
    pub not: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaFeatures {
    pub references: bool,
    pub definitions: bool,
    pub additional_properties: bool,
    pub const_values: bool,
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
