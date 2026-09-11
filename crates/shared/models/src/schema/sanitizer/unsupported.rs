//! Removal of constructs a provider does not accept: composition keywords,
//! references, metadata, `x-` extensions, and `const`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SchemaSanitizer;
use serde_json::{Map, Value, json};

impl SchemaSanitizer {
    pub(super) fn remove_unsupported_keywords(&self, obj: &mut Map<String, Value>) {
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
        if !self.capabilities.features.exclusive_bounds {
            obj.remove("exclusiveMinimum");
            obj.remove("exclusiveMaximum");
        }
        if !self.capabilities.features.property_names {
            obj.remove("propertyNames");
            obj.remove("patternProperties");
        }
        if !self.capabilities.features.tuple_items {
            Self::flatten_tuple_items(obj);
        }
    }

    // Why: Gemini's function_declarations reject `prefixItems` outright
    // ("Unknown name"), and Claude Code's tool schemas use tuple arrays such as
    // a `[field, operator, value]` triple. The array survives as a plain
    // `items` schema — the shared prefix schema when every position agrees,
    // otherwise an untyped item — instead of the whole request failing.
    pub(super) fn flatten_tuple_items(obj: &mut Map<String, Value>) {
        let prefix = obj.remove("prefixItems");
        obj.remove("additionalItems");
        obj.remove("unevaluatedItems");
        let Some(Value::Array(prefix)) = prefix else {
            return;
        };
        if obj.contains_key("items") {
            return;
        }
        let items = match prefix.split_first() {
            Some((first, rest)) if rest.iter().all(|s| s == first) => first.clone(),
            _ => Value::Object(Map::new()),
        };
        obj.insert("items".to_owned(), items);
    }

    pub(super) fn remove_metadata_fields(obj: &mut Map<String, Value>) {
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
            "$comment",
        ] {
            obj.remove(field);
        }
    }

    pub(super) fn remove_extension_fields(obj: &mut Map<String, Value>) {
        let extensions: Vec<String> = obj
            .keys()
            .filter(|k| k.starts_with("x-"))
            .cloned()
            .collect();
        for key in extensions {
            obj.remove(&key);
        }
    }

    pub(super) fn convert_const_to_enum(&self, obj: &mut Map<String, Value>) {
        if !self.capabilities.features.const_values
            && let Some(const_val) = obj.remove("const")
        {
            obj.insert("enum".to_owned(), json!([const_val]));
        }
        // Why: Vertex AI refuses a declaration node with `enum` but no `type`
        // ("schema didn't specify the schema type field"). A `const`-derived
        // enum never has one, and hand-written enums often omit it, so infer
        // it from the values when they agree.
        if !self.capabilities.features.const_values
            && !obj.contains_key("type")
            && let Some(Value::Array(values)) = obj.get("enum")
            && let Some(kind) = Self::common_json_type(values)
        {
            obj.insert("type".to_owned(), Value::String(kind.to_owned()));
        }
    }
}
