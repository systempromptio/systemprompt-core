use systemprompt_slack::events::{
    EventsApiEnvelope, InteractionPayload, SlackSurface, SlashCommand,
};

#[test]
fn url_verification_deserializes() {
    let env: EventsApiEnvelope =
        serde_json::from_str(r#"{"type":"url_verification","challenge":"abc123"}"#).unwrap();
    match env {
        EventsApiEnvelope::UrlVerification { challenge } => assert_eq!(challenge, "abc123"),
        EventsApiEnvelope::EventCallback { .. } => panic!("wrong variant"),
    }
}

#[test]
fn event_callback_deserializes() {
    let env: EventsApiEnvelope = serde_json::from_str(
        r#"{"type":"event_callback","team_id":"T1",
            "event":{"type":"app_mention","user":"U1","channel":"C1","text":"hi","ts":"123.45"}}"#,
    )
    .unwrap();
    match env {
        EventsApiEnvelope::EventCallback { team_id, event } => {
            assert_eq!(team_id.as_str(), "T1");
            assert_eq!(event.kind, "app_mention");
            assert_eq!(event.text.as_deref(), Some("hi"));
            assert_eq!(event.channel.as_ref().map(|c| c.as_str()), Some("C1"));
        },
        EventsApiEnvelope::UrlVerification { .. } => panic!("wrong variant"),
    }
}

#[test]
fn slash_command_normalizes_with_response_url() {
    let cmd: SlashCommand = serde_json::from_str(
        r#"{"command":"/ask","text":"hello there","user_id":"U1",
            "channel_id":"C1","team_id":"T1","response_url":"https://hooks.slack.com/x"}"#,
    )
    .unwrap();
    let n = cmd.normalize();
    assert_eq!(n.surface, SlackSurface::Command);
    assert_eq!(n.routing_key, "/ask");
    assert_eq!(n.text, "hello there");
    assert_eq!(n.workspace_id.as_str(), "T1");
    assert_eq!(n.channel_id.as_str(), "C1");
    assert_eq!(n.slack_user_id.as_str(), "U1");
    assert_eq!(n.response_url.as_deref(), Some("https://hooks.slack.com/x"));
}

#[test]
fn interaction_payload_deserializes() {
    let payload: InteractionPayload = serde_json::from_str(
        r#"{"type":"block_actions","user":{"id":"U1"},"channel":{"id":"C1"},
            "team":{"id":"T1"},"response_url":"https://hooks.slack.com/y",
            "actions":[{"action_id":"approve","value":"yes"}]}"#,
    )
    .unwrap();
    assert_eq!(payload.kind, "block_actions");
    assert_eq!(payload.user.id.as_str(), "U1");
    assert_eq!(payload.team.id.as_str(), "T1");
    assert_eq!(payload.actions.len(), 1);
    assert_eq!(payload.actions[0].action_id, "approve");
}
