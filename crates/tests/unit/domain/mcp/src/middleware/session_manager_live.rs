//! Full-lifecycle test for `DatabaseSessionHandler`'s `SessionManager` impl: a
//! real rmcp service is spawned on the session transport and the handler is
//! driven through create, initialize, request-stream, notification, standalone
//! stream, resume, and close.

use futures::StreamExt;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::streamable_http_server::session::{SessionId, SessionManager};
use rmcp::{ServerHandler, ServiceExt};
use systemprompt_mcp::middleware::DatabaseSessionHandler;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Clone)]
struct Quiet;

impl ServerHandler for Quiet {}

fn message(json: serde_json::Value) -> ClientJsonRpcMessage {
    serde_json::from_value(json).expect("valid client message")
}

fn initialize_message() -> ClientJsonRpcMessage {
    message(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "session-live-test", "version": "0.0.0"}
        }
    }))
}

#[tokio::test]
async fn session_manager_drives_full_streamable_http_lifecycle() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };

    let handler = DatabaseSessionHandler::new(&db);
    let (id, transport) = handler.create_session().await.expect("session created");
    let service = tokio::spawn(async move { Quiet.serve(transport).await });

    let response = handler
        .initialize_session(&id, initialize_message())
        .await
        .expect("initialize handled");
    let response_json = serde_json::to_value(&response).expect("serializable");
    assert_eq!(response_json["id"], 0, "got: {response_json}");
    assert!(
        response_json["result"]["protocolVersion"].is_string(),
        "got: {response_json}"
    );

    assert!(handler.has_session(&id).await.expect("has_session"));

    handler
        .accept_message(
            &id,
            message(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })),
        )
        .await
        .expect("initialized notification accepted");

    let mut stream = handler
        .create_stream(
            &id,
            message(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list"
            })),
        )
        .await
        .expect("request stream created");
    let mut response_seen = false;
    let mut observed = Vec::new();
    while let Some(sse) = stream.next().await {
        let body = serde_json::to_value(&sse.message).expect("serializable");
        if body["id"] == 1 {
            assert!(body["result"]["tools"].is_array(), "got: {body}");
            response_seen = true;
            break;
        }
        observed.push(body);
        if observed.len() > 16 {
            break;
        }
    }
    assert!(response_seen, "no tools/list response seen: {observed:?}");

    let standalone = handler
        .create_standalone_stream(&id)
        .await
        .expect("standalone stream created");
    drop(standalone);

    let resumed = handler.resume(&id, "0".to_owned()).await;
    assert!(
        resumed.is_ok(),
        "resume on a live session must recover: {:?}",
        resumed.err()
    );

    handler.close_session(&id).await.expect("session closed");
    assert!(!handler.has_session(&id).await.expect("has_session"));

    let missing = handler
        .resume(
            &SessionId::from(format!("gone-{}", uuid::Uuid::new_v4().simple())),
            "0".to_owned(),
        )
        .await;
    assert!(
        missing.is_err(),
        "resume of an unknown session must be rejected"
    );

    service.abort();
}
