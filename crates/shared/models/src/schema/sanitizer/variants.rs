//! Shapes Gemini and Vertex refuse outright: multi-type nodes, untyped
//! composition variants, and `items` that does not sit on an array.

use super::SchemaSanitizer;
use serde_json::{Map, Value, json};

impl SchemaSanitizer {
    // Why: Gemini's declaration schema has a single `type`; a JSON-Schema type
    // list such as `["array", "string"]` is refused outright. The list becomes
    // typed `anyOf` variants — appended to any variants already there — so a
    // later pass can pin `items` to the array variant instead of leaving it
    // beside a type Gemini will not accept it on.
    pub(super) fn split_type_list_into_variants(obj: &mut Map<String, Value>) {
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
    pub(super) fn type_or_drop_variants(obj: &mut Map<String, Value>) {
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
                Self::infer_type(inner).is_some_and(|kind| {
                    inner.insert("type".to_owned(), Value::String(kind.to_owned()));
                    if kind == "array" && !inner.contains_key("items") {
                        inner.insert("items".to_owned(), Value::Object(Map::new()));
                    }
                    true
                })
            });
            if variants.is_empty() {
                obj.remove(keyword);
            }
        }
    }

    pub(super) fn infer_type(inner: &Map<String, Value>) -> Option<&'static str> {
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
    // Why: Gemini rejects `items` on anything but an ARRAY and an ARRAY without
    // `items`, while JSON Schema allows `items` beside an `anyOf` whose array
    // variant carries none. Runs before the nested pass so the outer `items`
    // reaches a variant before that variant is given an empty one.
    pub(super) fn pin_items_to_arrays(obj: &mut Map<String, Value>) {
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
            Some(Value::Bool(_) | Value::Null) => {
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
}
