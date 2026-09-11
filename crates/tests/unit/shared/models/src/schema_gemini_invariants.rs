//! Every sanitised tool schema must be a shape Gemini accepts — stated as
//! rules, not as remembered error strings.
//!
//! `gemini_declaration_violations` is the rule set; the corpus below is every
//! shape that has reached a Gemini or Vertex 400 from a real client — the
//! Atlassian MCP catalog through OpenCode, Claude Code's tuple arrays, Pydantic
//! optionals — plus the shapes the sanitizer used to leave half-fixed. A new
//! failing shape is added to the corpus, and the sanitizer has to make it pass.

use serde_json::{Value, json};
use systemprompt_models::schema::{
    ProviderCapabilities, SchemaSanitizer, gemini_declaration_violations,
};

fn gemini() -> SchemaSanitizer {
    SchemaSanitizer::new(ProviderCapabilities::gemini())
}

fn property(shape: Value) -> Value {
    json!({"type": "object", "properties": {"source_types": shape}})
}

#[test]
fn the_checker_names_each_rejection_gemini_makes() {
    let cases: Vec<(&str, Value)> = vec![
        ("items on a non-array", json!({"type": "string", "items": {}})),
        ("array without an items object", json!({"type": "array"})),
        ("type is a list", json!({"type": ["array", "null"], "items": {}})),
        ("variant without a type", json!({"anyOf": [{"minItems": 1}]})),
        ("unknown keyword prefixItems", json!({"type": "array", "items": {}, "prefixItems": []})),
        ("unknown keyword $ref", json!({"$ref": "#/x"})),
    ];
    for (expected, schema) in cases {
        let found = gemini_declaration_violations(&schema);
        assert!(
            found.iter().any(|v| v.contains(expected)),
            "{expected}: got {found:?}"
        );
    }
    assert!(gemini_declaration_violations(&json!({"type": "array", "items": {}})).is_empty());
}

// Why: these are the shapes that have produced
// "field predicate failed: $type == Type.ARRAY" and "any_of[0].items: missing
// field" in production (Atlassian's `source_types` through OpenCode,
// 2026-09-05 and 2026-09-11). Each must sanitise to an accepted declaration.
#[test]
fn every_known_source_types_shape_sanitises_to_an_accepted_declaration() {
    let items = json!({"type": "string", "enum": ["confluence", "jira"]});
    let shapes: Vec<Value> = vec![
        json!({"items": items, "anyOf": [{"type": "array"}, {"type": "string"}]}),
        json!({"items": items, "anyOf": [{"type": "array"}, {"type": "null"}]}),
        json!({"items": items, "anyOf": [{"type": "array"}, {"type": "string"}, {"type": "null"}]}),
        json!({"anyOf": [{"type": "array", "items": items}, {"type": "null"}], "default": null}),
        json!({"type": ["array", "string"], "items": items}),
        json!({"type": ["array", "null"], "items": items}),
        json!({"type": ["array", "string"], "items": items, "anyOf": [{"type": "array"}, {"type": "string"}]}),
        json!({"type": "string", "items": items}),
        json!({"items": items}),
        json!({"items": items, "oneOf": [{"type": "array"}, {"type": "string"}]}),
        json!({"items": items, "anyOf": [{"minItems": 1}, {"type": "string"}]}),
        json!({"items": items, "anyOf": [{"$ref": "#/$defs/x"}, {"type": "string"}]}),
        json!({"anyOf": [{"anyOf": [{"type": "array"}, {"type": "string"}]}, {"type": "null"}], "items": items}),
        json!({"type": "array", "items": [items.clone()]}),
        json!({"type": "array", "items": {}, "prefixItems": [{"type": "string"}, {"type": "string"}]}),
        json!({"const": "jira"}),
    ];
    for shape in shapes {
        let out = gemini().sanitize(property(shape.clone()));
        let violations = gemini_declaration_violations(&out);
        assert!(
            violations.is_empty(),
            "shape {shape} sanitised to {out} which Gemini refuses: {violations:?}"
        );
    }
}

// Why: the two real Atlassian declarations at the indices the 2026-09-11
// error named (`function_declarations[86]` / `[87]`), plus the one tool in
// that catalog that uses `anyOf`, as the server sends them.
#[test]
fn the_atlassian_search_declarations_sanitise_cleanly() {
    let catalog = json!([
        {"type": "object", "required": ["cloudId", "cql"], "properties": {
            "cloudId": {"type": "string"},
            "cql": {"type": "string", "minLength": 1, "maxLength": 4096},
            "cqlcontext": {"type": "string"},
            "limit": {"type": "integer", "minimum": 1, "maximum": 100},
            "cursor": {"type": "string"}
        }},
        {"type": "object", "required": ["cloudId", "jql"], "properties": {
            "cloudId": {"type": "string"},
            "jql": {"type": "string"},
            "searchResultMode": {"type": "string", "enum": ["issues", "count", "all"]},
            "maxResults": {"type": "number"},
            "fields": {"type": "array", "items": {}},
            "view": {"type": "string", "enum": ["compact", "evidence", "full"]}
        }},
        {"type": "object", "properties": {
            "value": {"anyOf": [{"type": "string"}, {"type": "number"}, {"type": "boolean"},
                                {"type": "object", "additionalProperties": true},
                                {"type": "array", "items": {}}]}
        }}
    ]);
    for declaration in catalog.as_array().expect("array") {
        let out = gemini().sanitize(declaration.clone());
        assert_eq!(gemini_declaration_violations(&out), Vec::<String>::new(), "{out}");
    }
}

#[test]
fn a_type_list_becomes_typed_variants_and_items_follows_the_array() {
    let out = gemini().sanitize(json!({"type": ["array", "string"], "items": {"type": "integer"}}));
    assert_eq!(
        out,
        json!({"anyOf": [{"type": "array", "items": {"type": "integer"}}, {"type": "string"}]})
    );
}

#[test]
fn an_untyped_variant_is_typed_from_what_it_says_or_dropped() {
    let out = gemini().sanitize(json!({"anyOf": [
        {"minItems": 1}, {"properties": {"a": {"type": "string"}}}, {"enum": ["x", "y"]}, {}
    ]}));
    assert_eq!(
        out["anyOf"],
        json!([
            {"type": "array", "items": {}, "minItems": 1},
            {"type": "object", "properties": {"a": {"type": "string"}}},
            {"type": "string", "enum": ["x", "y"]}
        ])
    );
    let gone = gemini().sanitize(json!({"type": "string", "anyOf": [{}]}));
    assert!(gone.get("anyOf").is_none(), "{gone}");
}

// Why: Anthropic and OpenAI accept the JSON-Schema shapes as written; the
// Gemini rewrites must not leak into their wires.
#[test]
fn the_gemini_rewrites_do_not_apply_to_other_providers() {
    let schema = json!({"type": ["array", "string"], "items": {"type": "integer"},
                        "anyOf": [{"minItems": 1}]});
    for caps in [ProviderCapabilities::anthropic(), ProviderCapabilities::openai()] {
        let out = SchemaSanitizer::new(caps).sanitize(schema.clone());
        assert_eq!(out["type"], json!(["array", "string"]));
        assert_eq!(out["anyOf"], json!([{"minItems": 1}]));
    }
}
