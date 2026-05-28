use serde_json::json;
use systemprompt_models::ai::execution_plan::{
    ExecutionState, PlannedToolCall, PlanningResult, TemplateRef, ToolCallResult,
};

#[test]
fn planning_result_direct_response_helpers() {
    let r = PlanningResult::direct_response("hello");
    assert!(r.is_direct());
    assert!(!r.is_tool_calls());
    assert_eq!(r.tool_count(), 0);
}

#[test]
fn planning_result_tool_calls_helpers() {
    let calls = vec![
        PlannedToolCall::new("a", json!({})),
        PlannedToolCall::new("b", json!({})),
    ];
    let r = PlanningResult::tool_calls("because", calls);
    assert!(r.is_tool_calls());
    assert!(!r.is_direct());
    assert_eq!(r.tool_count(), 2);
}

#[test]
fn planning_result_serde_round_trip_for_direct() {
    let r = PlanningResult::direct_response("x");
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["type"], "direct_response");
    let _back: PlanningResult = serde_json::from_value(json).unwrap();
}

#[test]
fn planned_tool_call_new_assigns_fields() {
    let c = PlannedToolCall::new("get_user", json!({"id": 1}));
    assert_eq!(c.tool_name, "get_user");
    assert_eq!(c.arguments["id"], 1);
}

#[test]
fn tool_call_result_success_constructor() {
    let r = ToolCallResult::success(
        "get_user".to_owned(),
        json!({"id": 1}),
        json!({"name": "alice"}),
        150,
    );
    assert!(r.success);
    assert!(r.error.is_none());
    assert_eq!(r.duration_ms, 150);
    assert_eq!(r.output["name"], "alice");
}

#[test]
fn tool_call_result_failure_constructor() {
    let r = ToolCallResult::failure("get_user".to_owned(), json!({}), "not found", 50);
    assert!(!r.success);
    assert_eq!(r.error.as_deref(), Some("not found"));
    assert!(r.output.is_null());
}

#[test]
fn execution_state_new_defaults_to_empty() {
    let s = ExecutionState::new();
    assert!(s.results.is_empty());
    assert!(!s.halted);
    assert!(s.halt_reason.is_none());
}

#[test]
fn execution_state_add_result_halts_on_first_failure() {
    let mut s = ExecutionState::new();
    s.add_result(ToolCallResult::success(
        "a".to_owned(),
        json!({}),
        json!({}),
        10,
    ));
    assert!(!s.halted);

    s.add_result(ToolCallResult::failure(
        "b".to_owned(),
        json!({}),
        "boom",
        20,
    ));
    assert!(s.halted);
    assert_eq!(s.halt_reason.as_deref(), Some("boom"));

    s.add_result(ToolCallResult::failure(
        "c".to_owned(),
        json!({}),
        "second",
        5,
    ));
    assert_eq!(s.halt_reason.as_deref(), Some("boom"), "first failure wins");
    assert_eq!(s.results.len(), 3);
}

#[test]
fn execution_state_filters_and_total_duration() {
    let mut s = ExecutionState::new();
    s.add_result(ToolCallResult::success(
        "a".to_owned(),
        json!({}),
        json!({}),
        10,
    ));
    s.add_result(ToolCallResult::failure("b".to_owned(), json!({}), "x", 20));
    s.add_result(ToolCallResult::success(
        "c".to_owned(),
        json!({}),
        json!({}),
        30,
    ));
    assert_eq!(s.successful_results().len(), 2);
    assert_eq!(s.failed_results().len(), 1);
    assert_eq!(s.total_duration_ms(), 60);
}

#[test]
fn template_ref_parses_valid_template() {
    let t = TemplateRef::parse("$0.output.user.name").unwrap();
    assert_eq!(t.tool_index, 0);
    assert_eq!(t.field_path, vec!["user".to_owned(), "name".to_owned()]);
}

#[test]
fn template_ref_parses_single_field() {
    let t = TemplateRef::parse("$2.output.id").unwrap();
    assert_eq!(t.tool_index, 2);
    assert_eq!(t.field_path, vec!["id".to_owned()]);
}

#[test]
fn template_ref_rejects_invalid_syntax() {
    assert!(TemplateRef::parse("just a string").is_none());
    assert!(TemplateRef::parse("$0.output.").is_none());
    assert!(TemplateRef::parse("$x.output.field").is_none());
    assert!(TemplateRef::parse("0.output.field").is_none());
}

#[test]
fn template_ref_format_round_trips() {
    let t = TemplateRef {
        tool_index: 3,
        field_path: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
    };
    let s = t.format();
    assert_eq!(s, "$3.output.a.b.c");
    let back = TemplateRef::parse(&s).unwrap();
    assert_eq!(back.tool_index, 3);
    assert_eq!(back.field_path, t.field_path);
}
