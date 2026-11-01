use bytes::Bytes;
use http::StatusCode;
use serde_json::json;
use systemprompt_gateway::client::CanonicalInbound;
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::service::route_match::describe_route_match;
use systemprompt_identifiers::RouteId;
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_models::origin::InboundWireProtocol;

fn response() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "claude-x".into(),
        content: vec![CanonicalContent::text("hello")],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage {
            input_tokens: 3,
            output_tokens: 2,
            ..CanonicalUsage::default()
        },
        ..Default::default()
    }
}

#[test]
fn an_in_process_request_has_no_wire_body() {
    let inbound = CanonicalInbound;

    assert_eq!(inbound.wire(), InboundWireProtocol::Internal);
    assert_eq!(inbound.passthrough_wire(), None);
    assert!(inbound.parse_request(&Bytes::from_static(b"{}")).is_err());
}

#[test]
fn responses_events_and_errors_render_in_the_anthropic_shape() {
    let canonical = CanonicalInbound;
    let anthropic = AnthropicMessagesInbound;
    let snapshot = response();
    let start = CanonicalEvent::MessageStart {
        id: "msg_1".into(),
        model: String::new(),
        usage: CanonicalUsage::default(),
    };
    let stop = CanonicalEvent::MessageStop {
        id: "msg_1".into(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
        raw_finish_reason: None,
    };

    let body: serde_json::Value =
        serde_json::from_slice(&canonical.render_response(&snapshot)).expect("json");
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "hello");
    assert_eq!(
        canonical.render_response(&snapshot),
        anthropic.render_response(&snapshot)
    );
    assert_eq!(
        canonical.render_event(&start, "fallback"),
        anthropic.render_event(&start, "fallback")
    );
    assert!(canonical.render_event(&start, "fallback").is_some());
    assert_eq!(
        canonical.render_terminal_event(&stop, &snapshot, "claude-x"),
        anthropic.render_terminal_event(&stop, &snapshot, "claude-x")
    );
    let error = canonical.render_error(StatusCode::TOO_MANY_REQUESTS, "slow down");
    assert_eq!(
        error,
        anthropic.render_error(StatusCode::TOO_MANY_REQUESTS, "slow down")
    );
    assert!(String::from_utf8_lossy(&error).contains("slow down"));
}

#[test]
fn a_scoped_route_records_each_matched_dimension() {
    let route = GatewayRoute {
        when: Some(
            serde_json::from_value(json!({ "scope": { "department": "eng", "region": "eu" } }))
                .expect("route match"),
        ),
        ..GatewayRoute::new("scoped")
    };

    assert_eq!(
        describe_route_match(Some(&RouteId::new("scoped")), Some(&route), None, None),
        "route:scoped;scope:department=eng,region=eu"
    );
    assert_eq!(describe_route_match(None, None, None, None), "");
}
