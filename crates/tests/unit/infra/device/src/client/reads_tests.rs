use serde_json::json;
use systemprompt_device::client::ClientError;
use systemprompt_identifiers::PluginId;
use systemprompt_models::ApiPaths;

use crate::client::support::{FakeInstance, FakeResponse, jwt};

#[tokio::test]
async fn plugin_files_are_fetched_by_plain_paths() {
    let fake = FakeInstance::start(|request| match request.path() {
        "/v1/bridge/plugins/alpha-tools/agents/helper%20one.md" => {
            FakeResponse::text(200, "# helper")
        },
        _ => FakeResponse::status(404),
    })
    .await;
    let client = fake.client();
    let plugin = PluginId::new("alpha-tools");
    let bytes = client
        .plugin_file(&jwt(), &plugin, "agents/helper one.md")
        .await
        .unwrap();
    assert_eq!(bytes, b"# helper");
    for bad in ["../secrets", "agents//x", "a\\b", "", "./x"] {
        assert!(matches!(
            client.plugin_file(&jwt(), &plugin, bad).await,
            Err(ClientError::InvalidInput { .. })
        ));
    }
    let sneaky = PluginId::new("../other");
    assert!(matches!(
        client.plugin_file(&jwt(), &sneaky, "x").await,
        Err(ClientError::InvalidInput { .. })
    ));
    assert!(matches!(
        client.plugin_file(&jwt(), &plugin, "missing.md").await,
        Err(ClientError::Protocol {
            status: Some(404),
            ..
        })
    ));
}

#[tokio::test]
async fn reads_send_the_session_token_and_decode_the_answer() {
    let fake = FakeInstance::start(|request| match request.path() {
        ApiPaths::BRIDGE_WHOAMI => FakeResponse::text(200, "{ not json"),
        _ => FakeResponse::json(404, &json!({ "message": "no such route" })),
    })
    .await;
    let client = fake.client();
    let garbled = client.whoami(&jwt()).await.unwrap_err();
    assert!(matches!(
        garbled,
        ClientError::Protocol {
            status: Some(200),
            detail: None,
            ..
        }
    ));
    let sent = fake.last();
    assert_eq!(sent.method, "GET");
    assert_eq!(
        sent.header("authorization"),
        Some("Bearer header.claims.signature")
    );
    assert_eq!(sent.header("accept"), Some("application/json"));
    let missing = client.profile(&jwt()).await.unwrap_err();
    assert_eq!(missing.status(), Some(404));
    assert!(!missing.is_transient());
}
