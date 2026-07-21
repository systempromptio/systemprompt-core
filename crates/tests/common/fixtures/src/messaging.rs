//! Messaging-dispatch fixtures.
//!
//! [`seed_agent_backend`] registers a `running` agent-module `services` row
//! pointing at a wiremock backend, so the proxy resolves the dispatch target to
//! a loopback server. The agent itself resolves in the registry from the
//! fixture `config.yaml` ([`TEST_MESSAGING_AGENT`], `oauth.required = false`),
//! so the proxy forwards without a token check. The response builders return
//! the exact A2A JSON-RPC bodies `dispatch_messaging` parses.

use anyhow::Result;
use serde_json::{json, Value};
use systemprompt_database::{CreateServiceInput, DbPool, ServiceRepository};
use wiremock::MockServer;

use crate::bootstrap::TEST_MESSAGING_AGENT;

// Register the dispatchable agent backend at `mock`'s loopback port.
// Idempotent — `create_service` upserts on the service name, so a re-run
// repoints the row.
pub async fn seed_agent_backend(pool: &DbPool, mock: &MockServer) -> Result<()> {
    let repo = ServiceRepository::new(pool).map_err(|e| anyhow::anyhow!("service repo: {e}"))?;
    repo.create_service(CreateServiceInput {
        name: TEST_MESSAGING_AGENT,
        module_name: "agent",
        status: "running",
        port: mock.address().port(),
        binary_mtime: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("seed agent backend: {e}"))?;
    Ok(())
}

// A successful `message/send` response whose terminal status message carries
// `text` — the shape the dispatch reply extraction joins text parts from.
#[must_use]
pub fn agent_reply_response_json(text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "id": format!("task-{}", uuid::Uuid::new_v4()),
            "contextId": uuid::Uuid::new_v4().to_string(),
            "status": {
                "state": "TASK_STATE_COMPLETED",
                "message": {
                    "role": "ROLE_AGENT",
                    "parts": [{ "text": text }],
                    "messageId": uuid::Uuid::new_v4().to_string(),
                    "contextId": uuid::Uuid::new_v4().to_string()
                }
            }
        },
        "id": "1"
    })
}

// A JSON-RPC error response — drives the `MessagingError::Dispatch` branch.
#[must_use]
pub fn agent_error_response_json(code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message },
        "id": "1"
    })
}
