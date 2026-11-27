use systemprompt_device::protocol::wire::{
    HookTokenResponse, OAuthClientResponse, PatSession, SessionPatResponse, SyncTokenAnswer,
};
use systemprompt_identifiers::{ClientId, PluginId, SessionId};
use systemprompt_models::bridge::session::{
    Capabilities, Health, HookTokenRequest, SessionPatRequest,
};

#[test]
fn capabilities_tolerate_absence_and_unknown_modes() {
    let caps: Capabilities =
        serde_json::from_str(r#"{"modes":["pat","session","oauth-client","future"]}"#).unwrap();
    assert!(caps.offers(Capabilities::MODE_SESSION));
    assert!(!caps.offers("mtls"));
    let none: Capabilities = serde_json::from_str("{}").unwrap();
    assert_eq!(none.modes, Vec::<String>::new());
}

#[test]
fn health_reads_the_version() {
    let health: Health =
        serde_json::from_str(r#"{"status":"healthy","version":"0.76.0","extra":1}"#).unwrap();
    assert_eq!(health.version.as_deref(), Some("0.76.0"));
}

#[test]
fn session_pat_request_omits_a_missing_device_name_and_redacts() {
    let body = SessionPatRequest {
        code: "c0de",
        device_name: None,
    };
    assert_eq!(serde_json::to_string(&body).unwrap(), r#"{"code":"c0de"}"#);
    assert!(!format!("{body:?}").contains("c0de"));
    let named = SessionPatRequest {
        code: "c0de",
        device_name: Some("WS-01"),
    };
    assert_eq!(
        serde_json::to_string(&named).unwrap(),
        r#"{"code":"c0de","device_name":"WS-01"}"#
    );
}

#[test]
fn pat_session_reads_token_ttl_and_session() {
    let body = r#"{"token":"a.b.c","ttl":3600,"headers":{"x-client-id":"sp_bridge","x-session-id":"sess_9"},"device_token":"d.e.f","more":true}"#;
    let session: PatSession = serde_json::from_str(body).unwrap();
    assert_eq!(session.ttl, 3600);
    assert_eq!(session.token.expose(), "a.b.c");
    assert_eq!(session.session_id(), Some(SessionId::new("sess_9")));
    assert_eq!(
        session
            .device_token
            .as_ref()
            .map(systemprompt_device::protocol::Jwt::expose),
        Some("d.e.f")
    );
    assert!(!format!("{session:?}").contains("a.b.c"));
}

#[test]
fn an_empty_pat_is_refused() {
    assert!(serde_json::from_str::<SessionPatResponse>(r#"{"pat":""}"#).is_err());
    let ok: SessionPatResponse = serde_json::from_str(r#"{"pat":"sp-live-x"}"#).unwrap();
    assert_eq!(ok.pat.expose(), "sp-live-x");
}

#[test]
fn hook_token_request_fields() {
    let client = ClientId::new("client_1");
    let plugin = PluginId::new("astound-commons");
    let request = HookTokenRequest::new(&client, "s3cret", &plugin);
    let fields = request.fields();
    assert_eq!(fields[1], ("client_id", "client_1"));
    assert_eq!(fields[0], ("grant_type", "client_credentials"));
    assert_eq!(fields[3], ("scope", "hook:govern hook:track"));
    assert_eq!(fields[4], ("audience", "hook"));
    assert_eq!(fields[5], ("plugin_id", "astound-commons"));
    assert!(!format!("{request:?}").contains("s3cret"));
}

#[test]
fn oauth_client_and_hook_token_answers() {
    let client: OAuthClientResponse = serde_json::from_str(
        r#"{"client_id":"client_abc","client_secret":"sec","scopes":["hook:track"],"token_endpoint":"https://sp.example.com/api/v1/core/oauth/token"}"#,
    )
    .unwrap();
    assert_eq!(client.client_id.as_str(), "client_abc");
    assert_eq!(client.client_secret.expose(), "sec");
    let token: HookTokenResponse =
        serde_json::from_str(r#"{"access_token":"tok","token_type":"Bearer","expires_in":3600}"#)
            .unwrap();
    assert_eq!(token.expires_in, 3600);
    assert_eq!(token.access_token.expose(), "tok");
}

#[test]
fn the_sync_token_redacts_and_refuses_empty() {
    let answer: SyncTokenAnswer =
        serde_json::from_str(r#"{"token":"a.b.c","expires_at":"2026-11-01T09:00:00Z"}"#).unwrap();
    assert_eq!(answer.token.expose(), "a.b.c");
    assert!(!format!("{answer:?}").contains("a.b.c"));
    assert!(
        serde_json::from_str::<SyncTokenAnswer>(
            r#"{"token":"","expires_at":"2026-11-01T09:00:00Z"}"#
        )
        .is_err()
    );
}
