//! Tests for inbound activity normalization.

use systemprompt_teams::activities::{Activity, TeamsSurface};

fn message_json(text: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "message",
        "id": "act-1",
        "serviceUrl": "https://smba.trafficmanager.net/uk/",
        "text": text,
        "from": { "id": "29:user", "aadObjectId": "aad-123" },
        "conversation": { "id": "19:abc@thread.v2", "tenantId": "tenant-9" },
    })
}

#[test]
fn normalizes_a_message_with_aad_user_and_conversation_tenant() {
    let activity: Activity = serde_json::from_value(message_json("hello bot")).unwrap();
    let norm = activity.normalize().unwrap();

    assert_eq!(norm.surface, TeamsSurface::Message);
    assert_eq!(norm.tenant_id.as_str(), "tenant-9");
    assert_eq!(norm.conversation_id.as_str(), "19:abc@thread.v2");
    assert_eq!(norm.teams_user_id.as_str(), "aad-123");
    assert_eq!(norm.text, "hello bot");
    assert_eq!(norm.service_url, "https://smba.trafficmanager.net/uk/");
    assert_eq!(norm.activity_id.as_deref(), Some("act-1"));
}

#[test]
fn routing_key_is_conversation_id_for_plain_text() {
    let activity: Activity = serde_json::from_value(message_json("just chatting")).unwrap();
    let norm = activity.normalize().unwrap();
    assert_eq!(norm.routing_key, "19:abc@thread.v2");
}

#[test]
fn routing_key_is_the_leading_command_token() {
    let activity: Activity = serde_json::from_value(message_json("  /ask what is up")).unwrap();
    let norm = activity.normalize().unwrap();
    assert_eq!(norm.routing_key, "/ask");
}

#[test]
fn falls_back_to_from_id_when_no_aad_object_id() {
    let json = serde_json::json!({
        "type": "message",
        "serviceUrl": "https://smba.example/",
        "text": "hi",
        "from": { "id": "29:user-no-aad" },
        "conversation": { "id": "19:c", "tenantId": "t" },
    });
    let activity: Activity = serde_json::from_value(json).unwrap();
    let norm = activity.normalize().unwrap();
    assert_eq!(norm.teams_user_id.as_str(), "29:user-no-aad");
}

#[test]
fn reads_tenant_from_channel_data_when_conversation_has_none() {
    let json = serde_json::json!({
        "type": "invoke",
        "serviceUrl": "https://smba.example/",
        "from": { "id": "29:user" },
        "conversation": { "id": "19:c" },
        "channelData": { "tenant": { "id": "tenant-from-channeldata" } },
    });
    let activity: Activity = serde_json::from_value(json).unwrap();
    let norm = activity.normalize().unwrap();
    assert_eq!(norm.surface, TeamsSurface::Invoke);
    assert_eq!(norm.tenant_id.as_str(), "tenant-from-channeldata");
}

#[test]
fn rejects_an_activity_with_no_tenant() {
    let json = serde_json::json!({
        "type": "message",
        "serviceUrl": "https://smba.example/",
        "text": "hi",
        "from": { "id": "29:user" },
        "conversation": { "id": "19:c" },
    });
    let activity: Activity = serde_json::from_value(json).unwrap();
    assert!(activity.normalize().is_err());
}

#[test]
fn rejects_an_unhandled_activity_type() {
    let json = serde_json::json!({
        "type": "conversationUpdate",
        "serviceUrl": "https://smba.example/",
        "from": { "id": "29:user" },
        "conversation": { "id": "19:c", "tenantId": "t" },
    });
    let activity: Activity = serde_json::from_value(json).unwrap();
    assert!(activity.surface().is_none());
    assert!(activity.normalize().is_err());
}
