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

    // Why: Gemini's declaration schema has a single `type`; a JSON-Schema type
    // list such as `["array", "string"]` is refused outright. The list becomes
    // typed `anyOf` variants — appended to any variants already there — so a
    // later pass can pin `items` to the array variant instead of leaving it
    // beside a type Gemini will not accept it on.
    fn split_type_list_into_variants(obj: &mut Map<String, Value>) {
        let Some(Value::Array(types)) = obj.get("type").cloned() else {
            return;
        };
        let names: Vec<String> = types
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        obj.remove("type");
        if names.is_empty() {
            return;
        }
        if names.len() == 1 {
            obj.insert("type".to_owned(), Value::String(names[0].clone()));
            return;
        }
        let mut variants: Vec<Value> = match obj.remove("anyOf") {
            Some(Value::Array(existing)) => existing,
            _ => Vec::new(),
        };
        for name in names {
            let already = variants
                .iter()
                .any(|v| v.get("type").and_then(Value::as_str) == Some(name.as_str()));
            if !already {
                variants.push(json!({ "type": name }));
            }
        }
        obj.insert("anyOf".to_owned(), Value::Array(variants));
    }

    // Why: Vertex refuses a composition variant with no `type` ("schema didn't
    // specify the schema type field"). Stripping `$ref` or `const` upstream can
    // leave exactly that. The type is inferred from what the variant still
    // says about itself; a variant that says nothing is dropped, and a
    // keyword left with no variants goes with it.
    fn type_or_drop_variants(obj: &mut Map<String, Value>) {
        for keyword in ["anyOf", "oneOf", "allOf"] {
            let Some(Value::Array(variants)) = obj.get_mut(keyword) else {
                continue;
            };
            variants.retain_mut(|variant| {
                let Some(inner) = variant.as_object_mut() else {
                    return false;
                };
                if inner.contains_key("type")
                    || ["anyOf", "oneOf", "allOf"]
                        .iter()
                        .any(|k| inner.contains_key(*k))
                {
                    return true;
                }
                match Self::infer_type(inner) {
                    Some(kind) => {
                        inner.insert("type".to_owned(), Value::String(kind.to_owned()));
                        if kind == "array" && !inner.contains_key("items") {
                            inner.insert("items".to_owned(), Value::Object(Map::new()));
                        }
                        true
                    },
                    None => false,
                }
            });
            if variants.is_empty() {
                obj.remove(keyword);
            }
        }
    }

    fn infer_type(inner: &Map<String, Value>) -> Option<&'static str> {
        let has = |keys: &[&str]| keys.iter().any(|k| inner.contains_key(*k));
        if has(&["items", "minItems", "maxItems", "uniqueItems"]) {
            return Some("array");
        }
        if has(&["properties", "required", "additionalProperties"]) {
            return Some("object");
        }
        if let Some(Value::Array(values)) = inner.get("enum") {
            return Self::common_json_type(values);
        }
        if has(&["minimum", "maximum", "multipleOf"]) {
            return Some("number");
        }
        if has(&["minLength", "maxLength", "pattern", "format"]) {
            return Some("string");
        }
        None
    }

    fn normalize_nullable(obj: &mut Map<String, Value>) {
        if let Some(Value::Array(values)) = obj.get_mut("enum") {
            values.retain(|v| !v.is_null());
        }
        if let Some(Value::Array(types)) = obj.get("type").cloned() {
            let original_len = types.len();
            let mut non_null: Vec<Value> = types
                .into_iter()
                .filter(|v| v.as_str() != Some("null"))
                .collect();
            if non_null.len() < original_len {
                if non_null.len() == 1 {
                    obj.insert("type".to_owned(), non_null.remove(0));
                } else if non_null.is_empty() {
                    obj.remove("type");
                } else {
                    obj.insert("type".to_owned(), Value::Array(non_null));
                }
                obj.insert("nullable".to_owned(), Value::Bool(true));
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
                    obj.insert("anyOf".to_owned(), Value::Array(non_null));
                }
                obj.insert("nullable".to_owned(), Value::Bool(true));
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
    fn flatten_tuple_items(obj: &mut Map<String, Value>) {
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
            "$comment",
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

    fn common_json_type(values: &[Value]) -> Option<&'static str> {
        let mut kinds = values.iter().filter(|v| !v.is_null()).map(|v| match v {
            Value::String(_) => "string",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            Value::Array(_) => "array",
            Value::Object(_) | Value::Null => "object",
        });
        let first = kinds.next()?;
        kinds.all(|k| k == first).then_some(first)
    }

    // Why: Gemini rejects `items` on anything but an ARRAY and an ARRAY without
    // `items`, while JSON Schema allows `items` beside an `anyOf` whose array
    // variant carries none. Runs before the nested pass so the outer `items`
    // reaches a variant before that variant is given an empty one.
    fn pin_items_to_arrays(obj: &mut Map<String, Value>) {
        // Why: draft-4 tuple validation writes `items` as a list of schemas,
        // and draft-2020 allows a boolean; Gemini wants one item object. The
        // list collapses like `prefixItems` does, and a boolean becomes the
        // untyped item Gemini accepts for "anything".
        match obj.get("items") {
            Some(Value::Array(tuple)) => {
                let shared = match tuple.split_first() {
                    Some((first, rest)) if rest.iter().all(|s| s == first) => first.clone(),
                    _ => Value::Object(Map::new()),
                };
                obj.insert("items".to_owned(), shared);
            },
            Some(Value::Bool(_)) | Some(Value::Null) => {
                obj.insert("items".to_owned(), Value::Object(Map::new()));
            },
            _ => {},
        }
        let declared = obj.get("type").and_then(Value::as_str).map(str::to_owned);
        if let Some(items) = obj.get("items").cloned()
            && declared.as_deref() != Some("array")
        {
            let mut has_variants = false;
            for keyword in ["anyOf", "oneOf", "allOf"] {
                if let Some(Value::Array(variants)) = obj.get_mut(keyword) {
                    has_variants = true;
                    for variant in variants.iter_mut().filter_map(Value::as_object_mut) {
                        if variant.get("type").and_then(Value::as_str) == Some("array")
                            && !variant.contains_key("items")
                        {
                            variant.insert("items".to_owned(), items.clone());
                        }
                    }
                }
            }
            if declared.is_none() && !has_variants {
                obj.insert("type".to_owned(), json!("array"));
            } else {
                obj.remove("items");
            }
        }
        if obj.get("type").and_then(Value::as_str) == Some("array") && !obj.contains_key("items") {
            obj.insert("items".to_owned(), Value::Object(Map::new()));
        }
    }

    fn sanitize_nested_schemas(&self, obj: &mut Map<String, Value>) {
        self.sanitize_properties(obj);
        self.sanitize_items(obj);
        self.sanitize_prefix_items(obj);
        self.sanitize_composition_keywords(obj);
        self.sanitize_additional_properties(obj);
    }

    fn sanitize_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(properties) = obj.get_mut("properties")
            && let Some(props_obj) = properties.as_object_mut()
        {
            for value in props_obj.values_mut() {
                *value = self.sanitize(value.clone());
            }
        }
    }

    fn sanitize_prefix_items(&self, obj: &mut Map<String, Value>) {
        if let Some(Value::Array(prefix)) = obj.get_mut("prefixItems") {
            for item in prefix.iter_mut() {
                *item = self.sanitize(item.clone());
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
            if let Some(arr_val) = obj.get_mut(keyword)
                && let Some(arr) = arr_val.as_array_mut()
            {
                for item in arr.iter_mut() {
                    *item = self.sanitize(item.clone());
                }
            }
        }
    }

    fn sanitize_additional_properties(&self, obj: &mut Map<String, Value>) {
        if let Some(additional_props) = obj.get_mut("additionalProperties")
            && additional_props.is_object()
        {
            *additional_props = self.sanitize(additional_props.clone());
        }
    }
}
