//! Recursion into nested schemas: properties, items, composition variants
//! and `additionalProperties`.

use super::SchemaSanitizer;
use serde_json::{Map, Value};

impl SchemaSanitizer {
    pub(super) fn sanitize_nested_schemas(&self, obj: &mut Map<String, Value>) {
        self.sanitize_properties(obj);
        self.sanitize_items(obj);
        self.sanitize_prefix_items(obj);
        self.sanitize_composition_keywords(obj);
        self.sanitize_additional_properties(obj);
    }

    pub(super) fn sanitize_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(properties) = obj.get_mut("properties")
            && let Some(props_obj) = properties.as_object_mut()
        {
            for value in props_obj.values_mut() {
                *value = self.sanitize(value.clone());
            }
        }
    }

    pub(super) fn sanitize_prefix_items(&self, obj: &mut Map<String, Value>) {
        if let Some(Value::Array(prefix)) = obj.get_mut("prefixItems") {
            for item in prefix.iter_mut() {
                *item = self.sanitize(item.clone());
            }
        }
    }

    pub(super) fn sanitize_items(&self, obj: &mut Map<String, Value>) {
        if let Some(items) = obj.get_mut("items") {
            *items = self.sanitize(items.clone());
        }
    }

    pub(super) fn sanitize_composition_keywords(&self, obj: &mut Map<String, Value>) {
        for keyword in ["anyOf", "oneOf", "allOf"] {
            if let Some(arr_val) = obj.get_mut(keyword)
                && let Some(arr) = arr_val.as_array_mut()
            {
                for item in arr.iter_mut() {
                    *item = self.sanitize(item.clone());
                }
            }
        }
    }

    pub(super) fn sanitize_additional_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(additional_props) = obj.get_mut("additionalProperties")
            && additional_props.is_object()
        {
            *additional_props = self.sanitize(additional_props.clone());
        }
    }
}
