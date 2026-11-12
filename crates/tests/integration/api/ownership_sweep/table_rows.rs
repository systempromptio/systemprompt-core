use super::table::Family::{
    AgentRuns, Artifacts, Brain, BridgeNotifications, Contexts, Documents, Drive, Folders, Issues,
    McpExecutions, Notifications, OAuthClients, ProviderCredentials, Subscriptions, Tasks, Uploads,
    Webhooks,
};
use super::table::Payload::{Built, Empty, Json, Multipart, Raw};
use super::table::Shape::{CallerScoped, Create, Forbidden, Listing, NotFound};
use super::table::{Row, owned, owned_at};

const DOC: &str = "routes/documents/mod.rs::router";
const COLLAB: &str = "routes/documents/mod.rs::with_collab";
const FOLDER: &str = "routes/documents/mod.rs::folders_router";
const DRIVE: &str = "routes/documents/drive.rs::router";
const UPLOAD: &str = "routes/uploads/mod.rs::routes";
const ISSUE: &str = "routes/issues/mod.rs::router";
const RUN: &str = "routes/agent_runs/mod.rs::router";
const CONTEXT: &str = "routes/agent/contexts/mod.rs::router";
const WEBHOOK: &str = "routes/agent/contexts/mod.rs::webhook_router";
const TASK: &str = "routes/agent/mod.rs::tasks_router";
const ARTIFACT: &str = "routes/agent/mod.rs::artifacts_router";
const EXECUTION: &str = "routes/proxy/mcp/executions.rs::executions_router";
const NOTIFY: &str = "routes/notifications/mod.rs::router";
const BRIDGE_NOTIFY: &str = "routes/gateway/bridge_notifications.rs::routes";
const BYOK: &str = "routes/gateway/provider_credentials.rs::routes";
const BRAIN: &str = "routes/brain/mod.rs::router";
const CLIENTS: &str = "routes/oauth/clients.rs::router";

const SHARE: &str = r#"{"subject_kind":"user","subject_value":"sweep-nobody","level":"view"}"#;
const COMMIT: &str = r#"{"document_id":"$ID","path":"sweep-probe.md","content_sha256":"25be323556dad377abb57fe7ec8c4b99a6527f488dda28d0c9b686528659c909","size_bytes":6,"device_id":"sweep"}"#;
const PRESENCE: &str =
    r#"{"document_id":"$ID","state":"viewing","source_kind":"bridge","source_ref":"sweep"}"#;
const FACT_NOTE: &str = r#"{"reason":"sweep"}"#;

fn context_event(_context: &str) -> String {
    serde_json::to_string(&systemprompt_models::ContextEvent::System(
        systemprompt_models::SystemEventBuilder::heartbeat(),
    ))
    .expect("system event")
}

#[rustfmt::skip]
pub(super) const OWNED: &[Row] = &[
    owned(DOC, "GET", r#""/""#, Documents, Listing, Empty),
    owned(DOC, "POST", r#""/""#, Documents, Create, Json("{}")),
    owned(DOC, "POST", r#""/import""#, Documents, Create, Json("{}")),
    owned(DOC, "GET", r#""/{id}""#, Documents, NotFound, Empty),
    owned(DOC, "HEAD", r#""/{id}""#, Documents, NotFound, Empty),
    owned(DOC, "PATCH", r#""/{id}""#, Documents, NotFound, Json(r#"{"name":"renamed.md"}"#)),
    owned(DOC, "DELETE", r#""/{id}""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/content""#, Documents, NotFound, Empty),
    owned(DOC, "HEAD", r#""/{id}/content""#, Documents, NotFound, Empty),
    owned(DOC, "PUT", r#""/{id}/content""#, Documents, NotFound, Raw),
    owned(DOC, "POST", r#""/{id}/edits""#, Documents, NotFound, Json(r#"{"ops":[{"op":"append_markdown","markdown":"x"}]}"#)),
    owned(DOC, "GET", r#""/{id}/read""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/live""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/collab/awareness""#, Documents, NotFound, Json(r##"{"color":"#336699","mode":"viewing"}"##)),
    owned(DOC, "GET", r#""/{id}/comments""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/comments""#, Documents, NotFound, Json(r#"{"body":"x"}"#)),
    owned(DOC, "POST", r#""/{id}/comments/{comment_id}/resolve""#, Documents, NotFound, Empty),
    owned(DOC, "PATCH", r#""/{id}/comments/{comment_id}""#, Documents, NotFound, Json(r#"{"body":"x"}"#)),
    owned(DOC, "DELETE", r#""/{id}/comments/{comment_id}""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/export/docx""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/export/markdown""#, Documents, NotFound, Empty),
    owned_at(DOC, "GET", r#""/{id}/media""#, "/{id}/media?ref=x", (Documents, NotFound, Empty)),
    owned(DOC, "GET", r#""/{id}/access""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/agent-cap""#, Documents, NotFound, Empty),
    owned(DOC, "PUT", r#""/{id}/agent-cap""#, Documents, NotFound, Json(r#"{"cap":null}"#)),
    owned(DOC, "POST", r#""/{id}/transfer""#, Documents, NotFound, Json(r#"{"transfer_to":"$CALLER"}"#)),
    owned(DOC, "DELETE", r#""/{id}/grants/all""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/untrash""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/copy""#, Documents, NotFound, Json("{}")),
    owned(DOC, "POST", r#""/{id}/access-requests""#, Documents, NotFound, Json(r#"{"level":"view"}"#)),
    owned(DOC, "POST", r#""/{id}/access-requests/{rid}/grant""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/access-requests/{rid}/decline""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/revisions""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/proposals""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/proposals/{pid}""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/proposals/{pid}/accept""#, Documents, NotFound, Json("{}")),
    owned(DOC, "POST", r#""/{id}/proposals/{pid}/reject""#, Documents, NotFound, Json("{}")),
    owned(DOC, "GET", r#""/{id}/projection""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/preview""#, Documents, NotFound, Empty),
    owned(DOC, "GET", r#""/{id}/grants""#, Documents, NotFound, Empty),
    owned(DOC, "POST", r#""/{id}/grants""#, Documents, NotFound, Json(SHARE)),
    owned_at(DOC, "DELETE", r#""/{id}/grants""#, "/{id}/grants?grant_id={grant}", (Documents, NotFound, Empty)),
    owned(DOC, "GET", r#""/{id}/presence""#, Documents, NotFound, Empty),
    owned(DOC, "PUT", r#""/{id}/presence""#, Documents, NotFound, Json(r#"{"state":"viewing"}"#)),
    owned(DOC, "POST", r#""/{id}/lock""#, Documents, NotFound, Json(r#"{"holder_id":"sweep"}"#)),
    owned_at(DOC, "DELETE", r#""/{id}/lock""#, "/{id}/lock?holder_id=sweep", (Documents, NotFound, Empty)),
    owned(COLLAB, "POST", r#""/{id}/collab/join""#, Documents, NotFound, Json(r#"{"mode":"edit"}"#)),
    owned_at(COLLAB, "POST", r#""/{id}/collab/updates""#, "/{id}/collab/updates?session_id=8f14e45f-ceea-4e7a-9d3f-6a1e2b4c5d60", (Documents, NotFound, Raw)),
    owned(COLLAB, "POST", r#""/{id}/collab/leave""#, Documents, NotFound, Json(r#"{"session_id":"8f14e45f-ceea-4e7a-9d3f-6a1e2b4c5d60"}"#)),
    owned(COLLAB, "POST", r#""/{id}/collab/cursor""#, Documents, NotFound, Json(r#"{"session_id":"8f14e45f-ceea-4e7a-9d3f-6a1e2b4c5d60","cursor":{}}"#)),
    owned(FOLDER, "GET", r#""/""#, Folders, Listing, Empty),
    owned(FOLDER, "POST", r#""/""#, Folders, Create, Json(r#"{"name":"x"}"#)),
    owned(FOLDER, "PATCH", r#""/{id}""#, Folders, NotFound, Json(r#"{"name":"renamed"}"#)),
    owned(FOLDER, "DELETE", r#""/{id}""#, Folders, NotFound, Empty),
    owned(FOLDER, "GET", r#""/{id}/grants""#, Folders, NotFound, Empty),
    owned(FOLDER, "POST", r#""/{id}/grants""#, Folders, NotFound, Json(SHARE)),
    owned_at(FOLDER, "DELETE", r#""/{id}/grants""#, "/{id}/grants?grant_id={grant}", (Folders, NotFound, Empty)),
    owned(FOLDER, "GET", r#""/{id}/access""#, Folders, NotFound, Empty),
    owned(FOLDER, "GET", r#""/{id}/agent-cap""#, Folders, NotFound, Empty),
    owned(FOLDER, "PUT", r#""/{id}/agent-cap""#, Folders, NotFound, Json(r#"{"cap":null}"#)),
    owned_at(DRIVE, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_MANIFEST)", "/bridge/drive/manifest", (Drive, Listing, Empty)),
    owned_at(DRIVE, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_CHANGES)", "/bridge/drive/changes", (Drive, Listing, Empty)),
    owned_at(DRIVE, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_PROPOSALS)", "/bridge/drive/proposals", (Drive, Listing, Empty)),
    owned_at(DRIVE, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_BLOB_ROUTE)", "/bridge/drive/blob/{sha256}", (Drive, NotFound, Empty)),
    owned_at(DRIVE, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_COMMIT)", "/bridge/drive/commit", (Drive, NotFound, Multipart(COMMIT))),
    owned_at(DRIVE, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_TRASH)", "/bridge/drive/trash", (Drive, NotFound, Json(r#"{"document_id":"$ID"}"#))),
    owned_at(DRIVE, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_MOVE)", "/bridge/drive/move", (Drive, NotFound, Json(r#"{"document_id":"$ID","path":"moved.md"}"#))),
    owned_at(DRIVE, "PUT", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_PRESENCE)", "/bridge/drive/presence", (Drive, NotFound, Json(PRESENCE))),
    owned(UPLOAD, "POST", r#""/""#, Uploads, Create, Json("{}")),
    owned(UPLOAD, "GET", r#""/{id}""#, Uploads, NotFound, Empty),
    owned(UPLOAD, "DELETE", r#""/{id}""#, Uploads, NotFound, Empty),
    owned_at(UPLOAD, "PUT", r#""/{id}/parts/{part}""#, "/{id}/parts/1", (Uploads, NotFound, Raw)),
    owned(UPLOAD, "POST", r#""/{id}/complete""#, Uploads, NotFound, Empty),
    owned(UPLOAD, "GET", r#""/{id}/content""#, Uploads, NotFound, Empty),
    owned(UPLOAD, "HEAD", r#""/{id}/content""#, Uploads, NotFound, Empty),
    owned(ISSUE, "GET", r#""/""#, Issues, Listing, Empty),
    owned(ISSUE, "POST", r#""/""#, Issues, Create, Json("{}")),
    owned(ISSUE, "GET", r#""/{id}""#, Issues, NotFound, Empty),
    owned(ISSUE, "POST", r#""/{id}/attachments""#, Issues, NotFound, Json(r#"{"file_name":"a.txt","mime":"text/plain","size":6,"sha256":"25be323556dad377abb57fe7ec8c4b99a6527f488dda28d0c9b686528659c909"}"#)),
    owned(ISSUE, "POST", r#""/{id}/attachments/{upload_id}""#, Issues, NotFound, Empty),
    owned(RUN, "POST", r#""/""#, AgentRuns, Create, Json("{}")),
    owned(RUN, "GET", r#""/{id}""#, AgentRuns, NotFound, Empty),
    owned(RUN, "POST", r#""/{id}/cancel""#, AgentRuns, NotFound, Empty),
    owned(RUN, "GET", r#""/{id}/events""#, AgentRuns, NotFound, Empty),
    owned(RUN, "GET", r#""/{id}/events/stream""#, AgentRuns, NotFound, Empty),
    owned(RUN, "GET", r#""/{id}/outputs""#, AgentRuns, NotFound, Empty),
    owned(RUN, "GET", r#""/{id}/outputs/{*path}""#, AgentRuns, NotFound, Empty),
    owned(CONTEXT, "GET", r#""/""#, Contexts, Listing, Empty),
    owned(CONTEXT, "POST", r#""/""#, Contexts, Create, Json(r#"{"name":"x"}"#)),
    owned(CONTEXT, "GET", r#""/{id}""#, Contexts, NotFound, Empty),
    owned(CONTEXT, "PUT", r#""/{id}""#, Contexts, NotFound, Json(r#"{"name":"renamed"}"#)),
    owned(CONTEXT, "DELETE", r#""/{id}""#, Contexts, NotFound, Empty),
    owned(CONTEXT, "GET", r#""/{context_id}/tasks""#, Contexts, NotFound, Empty),
    owned(CONTEXT, "GET", r#""/{context_id}/artifacts""#, Contexts, NotFound, Empty),
    owned(CONTEXT, "POST", r#""/{context_id}/notifications""#, Contexts, NotFound, Json(r#"{"jsonrpc":"2.0","method":"notifications/taskStatusUpdate","params":{"agentId":"a","taskId":"t","status":{"state":"completed"},"task":{"id":"t"}}}"#)),
    owned(CONTEXT, "POST", r#""/{context_id}/events""#, Contexts, NotFound, Built(context_event)),
    owned_at(WEBHOOK, "POST", r#""/broadcast""#, "/broadcast", (Webhooks, Forbidden, Json(r#"{"event_type":"task_created","entity_id":"e","context_id":"$ID","user_id":"$CALLER"}"#))),
    owned(TASK, "GET", r#""/""#, Tasks, Listing, Empty),
    owned(TASK, "GET", r#""/{task_id}""#, Tasks, NotFound, Empty),
    owned(TASK, "DELETE", r#""/{task_id}""#, Tasks, NotFound, Empty),
    owned(TASK, "GET", r#""/{task_id}/messages""#, Tasks, NotFound, Empty),
    owned(TASK, "GET", r#""/{task_id}/artifacts""#, Tasks, NotFound, Empty),
    owned(ARTIFACT, "GET", r#""/""#, Artifacts, Listing, Empty),
    owned(ARTIFACT, "GET", r#""/{artifact_id}""#, Artifacts, NotFound, Empty),
    owned(ARTIFACT, "GET", r#""/{artifact_id}/ui""#, Artifacts, NotFound, Empty),
    owned(EXECUTION, "GET", r#""/executions/{id}""#, McpExecutions, NotFound, Empty),
    owned(NOTIFY, "GET", r#""/""#, Notifications, Listing, Empty),
    owned(NOTIFY, "GET", r#""/unread""#, Notifications, CallerScoped, Empty),
    owned(NOTIFY, "POST", r#""/read""#, Notifications, CallerScoped, Json(r#"{"ids":["$ID"]}"#)),
    owned(NOTIFY, "POST", r#""/seen""#, Notifications, CallerScoped, Json(r#"{"ids":["$ID"]}"#)),
    owned(NOTIFY, "POST", r#""/{id}/escalate""#, Notifications, NotFound, Json("{}")),
    owned(NOTIFY, "GET", r#""/streams""#, Notifications, CallerScoped, Empty),
    owned(NOTIFY, "GET", r#""/rules""#, Notifications, CallerScoped, Empty),
    owned(NOTIFY, "PUT", r#""/rules""#, Notifications, CallerScoped, Json(r#"{"rules":[]}"#)),
    owned(NOTIFY, "GET", r#""/subscriptions""#, Subscriptions, Listing, Empty),
    owned(NOTIFY, "POST", r#""/subscriptions""#, Subscriptions, Create, Json("{}")),
    owned(NOTIFY, "DELETE", r#""/subscriptions/{id}""#, Subscriptions, NotFound, Empty),
    owned_at(BRIDGE_NOTIFY, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS)", "/bridge/notifications", (BridgeNotifications, Listing, Empty)),
    owned_at(BRIDGE_NOTIFY, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_READ)", "/bridge/notifications/read", (BridgeNotifications, CallerScoped, Json(r#"{"ids":["$ID"]}"#))),
    owned_at(BRIDGE_NOTIFY, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN)", "/bridge/notifications/seen", (BridgeNotifications, CallerScoped, Json(r#"{"ids":["$ID"]}"#))),
    owned_at(BRIDGE_NOTIFY, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_ESCALATE_ROUTE)", "/bridge/notifications/{id}/escalate", (BridgeNotifications, NotFound, Json("{}"))),
    owned_at(BRIDGE_NOTIFY, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_STREAMS)", "/bridge/notifications/streams", (BridgeNotifications, CallerScoped, Empty)),
    owned_at(BRIDGE_NOTIFY, "GET", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS)", "/bridge/notifications/subscriptions", (BridgeNotifications, CallerScoped, Empty)),
    owned_at(BRIDGE_NOTIFY, "POST", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTIONS)", "/bridge/notifications/subscriptions", (BridgeNotifications, Create, Json("{}"))),
    owned_at(BRIDGE_NOTIFY, "DELETE", "ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS_SUBSCRIPTION_ROUTE)", "/bridge/notifications/subscriptions/{id}", (BridgeNotifications, NotFound, Empty)),
    owned(BYOK, "GET", r#""/me/provider-credentials""#, ProviderCredentials, CallerScoped, Empty),
    owned_at(BYOK, "PUT", r#""/me/provider-credentials/{credential}""#, "/me/provider-credentials/byok", (ProviderCredentials, CallerScoped, Json(r#"{"api_key":"good-key","label":"stranger"}"#))),
    owned_at(BYOK, "DELETE", r#""/me/provider-credentials/{credential}""#, "/me/provider-credentials/byok", (ProviderCredentials, CallerScoped, Empty)),
    owned(BRAIN, "GET", r#""/facts""#, Brain, Listing, Empty),
    owned(BRAIN, "POST", r#""/facts""#, Brain, Create, Json("{}")),
    owned(BRAIN, "GET", r#""/facts/{id}""#, Brain, NotFound, Empty),
    owned(BRAIN, "GET", r#""/facts/{id}/timeline""#, Brain, NotFound, Empty),
    owned(BRAIN, "GET", r#""/facts/{id}/why""#, Brain, NotFound, Empty),
    owned(BRAIN, "POST", r#""/facts/{id}/confirm""#, Brain, NotFound, Empty),
    owned(BRAIN, "POST", r#""/facts/{id}/dispute""#, Brain, NotFound, Json(FACT_NOTE)),
    owned(BRAIN, "POST", r#""/facts/{id}/retract""#, Brain, NotFound, Json(FACT_NOTE)),
    owned(BRAIN, "POST", r#""/facts/{id}/erase""#, Brain, NotFound, Empty),
    owned(BRAIN, "POST", r#""/facts/{id}/pin""#, Brain, NotFound, Empty),
    owned(BRAIN, "POST", r#""/facts/{id}/unpin""#, Brain, NotFound, Empty),
    owned_at(BRAIN, "POST", r#""/disputes/{id}/resolve""#, "/disputes/8f14e45f-ceea-4e7a-9d3f-6a1e2b4c5d60/resolve", (Brain, NotFound, Json(r#"{"fact_id":"$ID","outcome":"dismissed"}"#))),
    owned(BRAIN, "PATCH", r#""/facts/{id}/fields""#, Brain, NotFound, Json(r#"{"fields":{}}"#)),
    owned_at(BRAIN, "GET", r#""/subjects/{kind}/{id}""#, "/subjects/{subject}", (Brain, Listing, Empty)),
    owned_at(CLIENTS, "GET", r#""/""#, "/clients", (OAuthClients, Listing, Empty)),
    owned_at(CLIENTS, "POST", r#""/""#, "/clients", (OAuthClients, Create, Json("{}"))),
    owned_at(CLIENTS, "GET", r#""/{client_id}""#, "/clients/{client_id}", (OAuthClients, NotFound, Empty)),
    owned_at(CLIENTS, "PUT", r#""/{client_id}""#, "/clients/{client_id}", (OAuthClients, NotFound, Json(r#"{"name":"taken over","redirect_uris":["https://attacker.invalid/cb"],"scopes":["user"]}"#))),
    owned_at(CLIENTS, "DELETE", r#""/{client_id}""#, "/clients/{client_id}", (OAuthClients, NotFound, Empty)),
];
