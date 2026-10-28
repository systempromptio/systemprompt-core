use std::collections::BTreeMap;

use systemprompt_identifiers::{ModelId, ScopeDimension};
use systemprompt_manifest::services::gateway::{GatewayProfileError, RouteMatch};
use systemprompt_manifest::services::{ResponseFormatKind, RouteRequirements};
use systemprompt_wire::canonical::{
    CanonicalReasoning, CanonicalRequest, CanonicalSampling, CanonicalTool, CanonicalTooling,
    ReasoningEffort, ResponseFormat, ThinkingConfig,
};

fn tool(name: &str, kind: Option<&str>) -> CanonicalTool {
    CanonicalTool {
        name: name.to_owned(),
        anthropic_definition: kind.map(|k| serde_json::json!({ "type": k })),
        description: None,
        input_schema: serde_json::json!({}),
        cache_control: None,
    }
}

fn req(tools: Vec<CanonicalTool>) -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        cache_control: None,
        system: Vec::new(),
        messages: Vec::new(),
        max_tokens: 0,
        stream: false,
        metadata: None,
        response_format: None,
        forwarded_surface: Default::default(),
        sampling: CanonicalSampling {
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            presence_penalty: None,
            frequency_penalty: None,
        },
        tooling: CanonicalTooling {
            tools,
            tool_choice: None,
            search: None,
            code_execution: false,
        },
        reasoning: CanonicalReasoning {
            thinking: None,
            reasoning_effort: None,
        },
    }
}

#[test]
fn tool_name_predicate_needs_a_named_tool() {
    let when = RouteMatch {
        tool_names_any: vec!["search".to_owned()],
        ..RouteMatch::default()
    };
    assert!(when.matches(None, Some(&req(vec![tool("search", None)]))));
    assert!(!when.matches(None, Some(&req(vec![tool("other", None)]))));
    assert!(!when.matches(None, None));
    assert_eq!(when.matched_predicates(), vec!["tool_names_any"]);
}

#[test]
fn tool_type_predicate_matches_server_tool_prefixes_only() {
    let when = RouteMatch {
        tool_types_any: vec!["web_search".to_owned()],
        ..RouteMatch::default()
    };
    assert!(when.matches_request(&req(vec![tool("w", Some("web_search_20250305"))])));
    assert!(!when.matches_request(&req(vec![tool("w", Some("custom"))])));
    assert!(!when.matches_request(&req(vec![tool("w", None)])));
}

#[test]
fn tool_count_predicates() {
    let wants_tools = RouteMatch {
        requires_tools: Some(true),
        min_tools: Some(2),
        ..RouteMatch::default()
    };
    assert!(!wants_tools.matches_request(&req(vec![])));
    assert!(!wants_tools.matches_request(&req(vec![tool("a", None)])));
    assert!(wants_tools.matches_request(&req(vec![tool("a", None), tool("b", None)])));
    let no_tools = RouteMatch {
        requires_tools: Some(false),
        ..RouteMatch::default()
    };
    assert!(no_tools.matches_request(&req(vec![])));
    assert!(!no_tools.matches_request(&req(vec![tool("a", None)])));
}

#[test]
fn reasoning_stream_tokens_and_format_predicates() {
    let when = RouteMatch {
        thinking: Some(true),
        min_reasoning_effort: Some(ReasoningEffort::Medium),
        stream: Some(true),
        min_input_tokens: Some(1),
        response_format: Some(ResponseFormatKind::JsonObject),
        ..RouteMatch::default()
    };
    let mut request = req(vec![]);
    assert!(!when.matches_request(&request));
    request.reasoning.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: None,
    });
    request.reasoning.reasoning_effort = Some(ReasoningEffort::High);
    request.stream = true;
    request.response_format = Some(ResponseFormat::JsonObject);
    assert!(when.matches_request(&request));
    request.reasoning.reasoning_effort = Some(ReasoningEffort::Low);
    assert!(!when.matches_request(&request));
    request.reasoning.reasoning_effort = Some(ReasoningEffort::High);
    let heavier = RouteMatch {
        min_input_tokens: Some(1000),
        ..when.clone()
    };
    assert!(!heavier.matches_request(&request));
    assert_eq!(
        when.matched_predicates(),
        vec![
            "thinking",
            "min_reasoning_effort",
            "stream",
            "min_input_tokens",
            "response_format"
        ]
    );
    assert!(when.has_request_predicates());
}

#[test]
fn response_format_kind_from_wire() {
    assert_eq!(ResponseFormatKind::from(None), ResponseFormatKind::Text);
    assert_eq!(
        ResponseFormatKind::from(Some(&ResponseFormat::JsonObject)),
        ResponseFormatKind::JsonObject
    );
    let schema = ResponseFormat::JsonSchema {
        name: "s".to_owned(),
        schema: serde_json::json!({}),
        strict: true,
    };
    assert_eq!(
        ResponseFormatKind::from(Some(&schema)),
        ResponseFormatKind::JsonSchema
    );
}

#[test]
fn validate_refuses_each_malformed_shape() {
    let mut scope = BTreeMap::new();
    scope.insert(ScopeDimension::new("project"), "  ".to_owned());
    let cases = [
        (
            RouteMatch {
                scope,
                ..RouteMatch::default()
            },
            "scope",
        ),
        (
            RouteMatch {
                min_tools: Some(0),
                ..RouteMatch::default()
            },
            "zero",
        ),
        (
            RouteMatch {
                requires_tools: Some(false),
                min_tools: Some(1),
                ..RouteMatch::default()
            },
            "contradictory",
        ),
        (
            RouteMatch {
                tool_types_any: vec!["web search".to_owned()],
                ..RouteMatch::default()
            },
            "bad",
        ),
    ];
    for (when, label) in cases {
        let error = when.validate().expect_err(label);
        let ok = match label {
            "scope" => matches!(error, GatewayProfileError::RouteScopeValueEmpty),
            "zero" => matches!(error, GatewayProfileError::RouteMatchZeroMinTools),
            "contradictory" => matches!(error, GatewayProfileError::RouteMatchContradictoryTools),
            _ => {
                matches!(error, GatewayProfileError::RouteMatchBadToolType(ref t) if t == "web search")
            },
        };
        assert!(ok, "{label}: {error:?}");
    }
    assert!(RouteMatch::default().validate().is_ok());
}

#[test]
fn requirements_report_each_unmet_governance_flag() {
    let both = RouteRequirements {
        european: true,
        no_retain: true,
    };
    let unmet = both.unmet(Default::default());
    assert_eq!(unmet, vec!["european", "no_retain"]);
    assert_eq!(both.declared(), vec!["european", "no_retain"]);
    assert!(RouteRequirements::default().declared().is_empty());
}
