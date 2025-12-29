use super::capabilities::ProviderCapabilities;
use serde_json::{json, Map, Value};

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

        self.remove_unsupported_keywords(obj);
        Self::remove_metadata_fields(obj);
        Self::remove_extension_fields(obj);
        self.convert_const_to_enum(obj);
        self.sanitize_nested_schemas(obj);

        sanitized
    }

    fn remove_unsupported_keywords(&self, obj: &mut Map<String, Value>) {
        if !self.capabilities.supports_allof {
            obj.remove("allOf");
        }
        if !self.capabilities.supports_anyof {
            obj.remove("anyOf");
        }
        if !self.capabilities.supports_oneof {
            obj.remove("oneOf");
        }
        if !self.capabilities.supports_if_then_else {
            obj.remove("if");
            obj.remove("then");
            obj.remove("else");
        }
        if !self.capabilities.supports_ref {
            obj.remove("$ref");
        }
        if !self.capabilities.supports_definitions {
            obj.remove("definitions");
            obj.remove("$defs");
        }
        if !self.capabilities.supports_not {
            obj.remove("not");
        }
        if !self.capabilities.supports_additional_properties {
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
        if !self.capabilities.supports_const {
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
