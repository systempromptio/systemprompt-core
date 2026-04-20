use super::capabilities::ProviderCapabilities;
use serde_json::{Map, Value, json};

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
        self.sanitize_nested_schemas(obj);

        sanitized
    }

    fn normalize_nullable(obj: &mut Map<String, Value>) {
        if let Some(Value::Array(types)) = obj.get("type").cloned() {
            let original_len = types.len();
            let mut non_null: Vec<Value> = types
                .into_iter()
                .filter(|v| v.as_str() != Some("null"))
                .collect();
            if non_null.len() < original_len {
                if non_null.len() == 1 {
                    obj.insert("type".to_string(), non_null.remove(0));
                } else if non_null.is_empty() {
                    obj.remove("type");
                } else {
                    obj.insert("type".to_string(), Value::Array(non_null));
                }
                obj.insert("nullable".to_string(), Value::Bool(true));
            }
        }

        if let Some(Value::Array(variants)) = obj.get("anyOf").cloned() {
            let null_count = variants
                .iter()
                .filter(|v| v.get("type").and_then(Value::as_str) == Some("null"))
                .count();
            if null_count > 0 && null_count < variants.len() {
                let non_null: Vec<Value> = variants
                    .into_iter()
                    .filter(|v| v.get("type").and_then(Value::as_str) != Some("null"))
                    .collect();
                if non_null.len() == 1 {
                    obj.remove("anyOf");
                    if let Some(Value::Object(inner)) = non_null.into_iter().next() {
                        for (k, v) in inner {
                            obj.insert(k, v);
                        }
                    }
                } else {
                    obj.insert("anyOf".to_string(), Value::Array(non_null));
                }
                obj.insert("nullable".to_string(), Value::Bool(true));
            }
        }
    }

    fn remove_unsupported_keywords(&self, obj: &mut Map<String, Value>) {
        if !self.capabilities.composition.allof {
            obj.remove("allOf");
        }
        if !self.capabilities.composition.anyof {
            obj.remove("anyOf");
        }
        if !self.capabilities.composition.oneof {
            obj.remove("oneOf");
        }
        if !self.capabilities.composition.if_then_else {
            obj.remove("if");
            obj.remove("then");
            obj.remove("else");
        }
        if !self.capabilities.features.references {
            obj.remove("$ref");
        }
        if !self.capabilities.features.definitions {
            obj.remove("definitions");
            obj.remove("$defs");
        }
        if !self.capabilities.composition.not {
            obj.remove("not");
        }
        if !self.capabilities.features.additional_properties {
            obj.remove("additionalProperties");
        }
    }

    fn remove_metadata_fields(obj: &mut Map<String, Value>) {
        for field in [
            "$schema",
            "$id",
            "readOnly",
            "writeOnly",
            "deprecated",
            "examples",
            "contentMediaType",
            "contentEncoding",
            "outputSchema",
        ] {
            obj.remove(field);
        }
    }

    fn remove_extension_fields(obj: &mut Map<String, Value>) {
        let extensions: Vec<String> = obj
            .keys()
            .filter(|k| k.starts_with("x-"))
            .cloned()
            .collect();
        for key in extensions {
            obj.remove(&key);
        }
    }

    fn convert_const_to_enum(&self, obj: &mut Map<String, Value>) {
        if !self.capabilities.features.const_values {
            if let Some(const_val) = obj.remove("const") {
                obj.insert("enum".to_string(), json!([const_val]));
            }
        }
    }

    fn sanitize_nested_schemas(&self, obj: &mut Map<String, Value>) {
        self.sanitize_properties(obj);
        self.sanitize_items(obj);
        self.sanitize_composition_keywords(obj);
        self.sanitize_additional_properties(obj);
    }

    fn sanitize_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(properties) = obj.get_mut("properties") {
            if let Some(props_obj) = properties.as_object_mut() {
                for value in props_obj.values_mut() {
                    *value = self.sanitize(value.clone());
                }
            }
        }
    }

    fn sanitize_items(&self, obj: &mut Map<String, Value>) {
        if let Some(items) = obj.get_mut("items") {
            *items = self.sanitize(items.clone());
        }
    }

    fn sanitize_composition_keywords(&self, obj: &mut Map<String, Value>) {
        for keyword in ["anyOf", "oneOf", "allOf"] {
            if let Some(arr_val) = obj.get_mut(keyword) {
                if let Some(arr) = arr_val.as_array_mut() {
                    for item in arr.iter_mut() {
                        *item = self.sanitize(item.clone());
                    }
                }
            }
        }
    }

    fn sanitize_additional_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(additional_props) = obj.get_mut("additionalProperties") {
            if additional_props.is_object() {
                *additional_props = self.sanitize(additional_props.clone());
            }
        }
    }
}
