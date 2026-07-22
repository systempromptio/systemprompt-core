use serde_json::json;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::ai::{McpTool, PlannedToolCall, TemplateValidator, ValidationErrorKind};

fn tool(name: &str, output_schema: Option<serde_json::Value>) -> McpTool {
    McpTool {
        name: name.to_owned(),
        description: None,
        input_schema: None,
        output_schema,
        service_id: McpServerId::new("svc"),
        terminal_on_success: false,
        model_config: None,
    }
}

fn schema_with(fields: &[&str]) -> serde_json::Value {
    let props: serde_json::Map<String, serde_json::Value> = fields
        .iter()
        .map(|f| ((*f).to_owned(), json!({"type": "string"})))
        .collect();
    json!({"type": "object", "properties": props})
}

#[test]
fn valid_backward_reference_passes() {
    let calls = vec![
        PlannedToolCall::new("search", json!({"query": "rust"})),
        PlannedToolCall::new("summarize", json!({"text": "$0.output.results"})),
    ];
    let tools = vec![
        tool("search", Some(schema_with(&["results"]))),
        tool("summarize", Some(schema_with(&["summary"]))),
    ];
    let schemas = TemplateValidator::get_tool_output_schemas(&calls, &tools);
    assert!(TemplateValidator::validate_plan(&calls, &schemas).is_ok());
}

#[test]
fn self_reference_is_rejected() {
    let calls = vec![PlannedToolCall::new(
        "loop",
        json!({"input": "$0.output.value"}),
    )];
    let schemas = vec![("loop".to_owned(), Some(schema_with(&["value"])))];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].error,
        ValidationErrorKind::SelfReference
    ));
    assert_eq!(errors[0].tool_index, 0);
    assert_eq!(errors[0].argument, "input");
    assert!(errors[0].to_string().contains("cannot reference itself"));
}

#[test]
fn forward_reference_is_rejected() {
    let calls = vec![
        PlannedToolCall::new("first", json!({"input": "$1.output.value"})),
        PlannedToolCall::new("second", json!({})),
    ];
    let schemas = vec![
        ("first".to_owned(), Some(schema_with(&["value"]))),
        ("second".to_owned(), Some(schema_with(&["value"]))),
    ];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    assert!(matches!(
        errors[0].error,
        ValidationErrorKind::ForwardReference {
            referenced_index: 1
        }
    ));
    assert!(errors[0].to_string().contains("hasn't executed yet"));
}

#[test]
fn index_out_of_bounds_when_schemas_shorter_than_plan() {
    let calls = vec![
        PlannedToolCall::new("a", json!({})),
        PlannedToolCall::new("b", json!({})),
        PlannedToolCall::new("c", json!({"input": "$1.output.value"})),
    ];
    let schemas = vec![("a".to_owned(), Some(schema_with(&["value"])))];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    match &errors[0].error {
        ValidationErrorKind::IndexOutOfBounds {
            referenced_index,
            max_valid_index,
        } => {
            assert_eq!(*referenced_index, 1);
            assert_eq!(*max_valid_index, 0);
        },
        other => panic!("expected IndexOutOfBounds, got {other:?}"),
    }
    assert!(errors[0].to_string().contains("only tools 0-0"));
}

#[test]
fn invalid_template_syntax_is_rejected() {
    let calls = vec![
        PlannedToolCall::new("a", json!({})),
        PlannedToolCall::new("b", json!({"input": "$x.output.value"})),
    ];
    let schemas = vec![
        ("a".to_owned(), Some(schema_with(&["value"]))),
        ("b".to_owned(), None),
    ];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    assert!(matches!(
        errors[0].error,
        ValidationErrorKind::InvalidTemplateSyntax
    ));
    assert!(errors[0].to_string().contains("Invalid template syntax"));
}

#[test]
fn field_not_found_lists_available_fields() {
    let calls = vec![
        PlannedToolCall::new("search", json!({})),
        PlannedToolCall::new("use", json!({"input": "$0.output.missing"})),
    ];
    let schemas = vec![
        (
            "search".to_owned(),
            Some(schema_with(&["results", "count"])),
        ),
        ("use".to_owned(), None),
    ];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    match &errors[0].error {
        ValidationErrorKind::FieldNotFound {
            tool_name,
            field,
            available_fields,
        } => {
            assert_eq!(tool_name, "search");
            assert_eq!(field, "missing");
            assert!(available_fields.contains(&"results".to_owned()));
            assert!(available_fields.contains(&"count".to_owned()));
        },
        other => panic!("expected FieldNotFound, got {other:?}"),
    }
    let rendered = errors[0].to_string();
    assert!(rendered.contains("'missing'"));
    assert!(rendered.contains("search"));
}

#[test]
fn no_output_schema_is_rejected() {
    let calls = vec![
        PlannedToolCall::new("opaque", json!({})),
        PlannedToolCall::new("use", json!({"input": "$0.output.value"})),
    ];
    let schemas = vec![("opaque".to_owned(), None), ("use".to_owned(), None)];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    assert!(matches!(
        &errors[0].error,
        ValidationErrorKind::NoOutputSchema { tool_name } if tool_name == "opaque"
    ));
    assert!(errors[0].to_string().contains("has no output schema"));
}

#[test]
fn nested_template_argument_path_is_reported() {
    let calls = vec![
        PlannedToolCall::new("a", json!({})),
        PlannedToolCall::new(
            "b",
            json!({"outer": {"inner": "$0.output.value"}, "list": ["$0.output.value"]}),
        ),
    ];
    let schemas = vec![("a".to_owned(), None), ("b".to_owned(), None)];
    let errors = TemplateValidator::validate_plan(&calls, &schemas).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().any(|e| e.argument == "outer.inner"));
}

#[test]
fn find_templates_walks_nested_values_and_ignores_plain_strings() {
    let value = json!({
        "a": "$0.output.x",
        "b": ["$1.output.y", "not a template", "$literal"],
        "c": {"d": "$2.output.z.deep"},
        "e": 42,
        "f": null
    });
    let mut found = TemplateValidator::find_templates_in_value(&value);
    found.sort();
    assert_eq!(
        found,
        vec!["$0.output.x", "$1.output.y", "$2.output.z.deep"]
    );
}

#[test]
fn get_tool_output_schemas_matches_by_name() {
    let calls = vec![
        PlannedToolCall::new("known", json!({})),
        PlannedToolCall::new("unknown", json!({})),
    ];
    let tools = vec![tool("known", Some(schema_with(&["out"])))];
    let schemas = TemplateValidator::get_tool_output_schemas(&calls, &tools);
    assert_eq!(schemas.len(), 2);
    assert_eq!(schemas[0].0, "known");
    assert!(schemas[0].1.is_some());
    assert_eq!(schemas[1].0, "unknown");
    assert!(schemas[1].1.is_none());
}

#[test]
fn error_kind_serde_uses_snake_case_tag() {
    let kind = ValidationErrorKind::ForwardReference {
        referenced_index: 3,
    };
    let value = serde_json::to_value(&kind).unwrap();
    assert_eq!(value["type"], "forward_reference");
    assert_eq!(value["referenced_index"], 3);
    let back: ValidationErrorKind = serde_json::from_value(value).unwrap();
    assert!(matches!(
        back,
        ValidationErrorKind::ForwardReference {
            referenced_index: 3
        }
    ));
}
