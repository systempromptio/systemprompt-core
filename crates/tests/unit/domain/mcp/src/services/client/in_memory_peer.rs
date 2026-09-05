//! Runs a real rmcp client and server against each other over an in-process
//! duplex stream, so the `McpClientHandler` callbacks and the server-side
//! progress callback are exercised through the wire rather than called
//! directly.

use std::sync::Arc;

use futures::StreamExt;
use rmcp::model::{
    ClientInfo, ElicitRequestParams, ElicitResult, ElicitationAction, ElicitationSchema,
    Implementation, ProgressToken, ServerInfo,
};
use rmcp::service::{RoleClient, RoleServer, RunningService};
use rmcp::{ServerHandler, ServiceExt};
use systemprompt_mcp::create_progress_callback;
use systemprompt_mcp::services::client::{
    ElicitationDelegate, McpClientHandler, SharedElicitationDelegate,
};

#[derive(Debug, Clone)]
struct QuietServer;

impl ServerHandler for QuietServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::new("in-memory-probe", "1.0.0");
        info
    }
}

#[derive(Debug)]
struct FixedDelegate {
    action: ElicitationAction,
}

#[async_trait::async_trait]
impl ElicitationDelegate for FixedDelegate {
    async fn elicit(&self, _params: ElicitRequestParams) -> ElicitResult {
        ElicitResult::new(self.action.clone()).with_content(serde_json::json!({"from": "delegate"}))
    }
}

fn client_info() -> ClientInfo {
    ClientInfo::new(
        rmcp::model::ClientCapabilities::builder()
            .enable_elicitation()
            .build(),
        Implementation::new("in-memory-client", "9.9.9"),
    )
}

type Pair = (
    RunningService<RoleClient, McpClientHandler>,
    RunningService<RoleServer, QuietServer>,
);

async fn connected(handler: McpClientHandler) -> Pair {
    let (client_side, server_side) = tokio::io::duplex(16 * 1024);
    let server_task = tokio::spawn(async move { QuietServer.serve(server_side).await });
    let client = handler.serve(client_side).await.expect("client handshake");
    let server = server_task
        .await
        .expect("server task joins")
        .expect("server handshake");
    (client, server)
}

fn form_request(message: &str) -> ElicitRequestParams {
    ElicitRequestParams::FormElicitationParams {
        meta: None,
        message: message.to_owned(),
        requested_schema: ElicitationSchema::new(std::collections::BTreeMap::new()),
    }
}

#[tokio::test]
async fn the_client_handler_reports_the_client_info_it_was_built_with() {
    let handler = McpClientHandler::new(client_info());
    let (client, server) = connected(handler).await;

    let seen = server.peer_info().expect("client info negotiated");
    assert_eq!(
        seen.client_info.name, "in-memory-client",
        "get_info must hand the configured implementation to the server"
    );
    assert_eq!(seen.client_info.version, "9.9.9");

    let _ = client.cancel().await;
    let _ = server.cancel().await;
}

#[tokio::test]
async fn a_progress_notification_sent_by_the_server_reaches_the_matching_subscriber() {
    let handler = McpClientHandler::new(client_info());
    let dispatcher = handler.progress_dispatcher().clone();
    let (client, server) = connected(handler).await;

    let token = ProgressToken(rmcp::model::NumberOrString::String(Arc::from("job-1")));
    let mut subscriber = dispatcher.subscribe(token.clone()).await;

    let callback = create_progress_callback(token.clone(), server.peer().clone());
    callback(0.5, Some(1.0), Some("halfway".to_owned())).await;

    let received = tokio::time::timeout(std::time::Duration::from_secs(5), subscriber.next())
        .await
        .expect("notification arrives before the timeout")
        .expect("subscriber stream yields");

    assert_eq!(received.progress_token, token);
    assert!(
        (received.progress - 0.5).abs() < f64::EPSILON,
        "progress value must survive the wire: {}",
        received.progress
    );
    assert_eq!(received.total, Some(1.0));
    assert_eq!(received.message.as_deref(), Some("halfway"));

    drop(subscriber);
    let _ = client.cancel().await;
    let _ = server.cancel().await;
}

#[tokio::test]
async fn a_progress_notification_for_an_unsubscribed_token_is_dropped_not_misrouted() {
    let handler = McpClientHandler::new(client_info());
    let dispatcher = handler.progress_dispatcher().clone();
    let (client, server) = connected(handler).await;

    let watched = ProgressToken(rmcp::model::NumberOrString::String(Arc::from("watched")));
    let other = ProgressToken(rmcp::model::NumberOrString::String(Arc::from("other")));
    let mut subscriber = dispatcher.subscribe(watched).await;

    let callback = create_progress_callback(other, server.peer().clone());
    callback(0.25, None, None).await;

    let outcome =
        tokio::time::timeout(std::time::Duration::from_millis(300), subscriber.next()).await;
    assert!(
        outcome.is_err(),
        "a notification for a different token must not be delivered here"
    );

    drop(subscriber);
    let _ = client.cancel().await;
    let _ = server.cancel().await;
}

// Why: the callback is handed to tool code that keeps running after a client
// disconnects. `notify_progress` then fails, and the callback must absorb that
// rather than propagate it into the tool.
#[tokio::test]
async fn a_progress_callback_whose_peer_is_gone_completes_without_propagating_the_failure() {
    let handler = McpClientHandler::new(client_info());
    let (client, server) = connected(handler).await;

    let token = ProgressToken(rmcp::model::NumberOrString::String(Arc::from("dead")));
    let callback = create_progress_callback(token, server.peer().clone());

    let _ = client.cancel().await;
    let _ = server.cancel().await;

    let outcome =
        tokio::time::timeout(std::time::Duration::from_secs(5), callback(1.0, None, None)).await;
    assert!(
        outcome.is_ok(),
        "the callback must return even when the peer is unreachable"
    );
}

#[tokio::test]
async fn an_elicitation_request_is_declined_when_no_delegate_is_installed() {
    let handler = McpClientHandler::new(client_info());
    let (client, server) = connected(handler).await;

    let result = server
        .peer()
        .create_elicitation(form_request("share your address?"))
        .await
        .expect("client answers the elicitation");

    assert_eq!(
        result.action,
        ElicitationAction::Decline,
        "with no delegate the handler must decline rather than error"
    );
    assert!(result.content.is_none());

    let _ = client.cancel().await;
    let _ = server.cancel().await;
}

#[tokio::test]
async fn an_installed_delegates_answer_is_returned_to_the_server_over_the_wire() {
    let delegate: SharedElicitationDelegate = Arc::new(FixedDelegate {
        action: ElicitationAction::Accept,
    });
    let handler = McpClientHandler::new(client_info()).with_elicitation(delegate);
    let (client, server) = connected(handler).await;

    let result = server
        .peer()
        .create_elicitation(form_request("name?"))
        .await
        .expect("client answers the elicitation");

    assert_eq!(result.action, ElicitationAction::Accept);
    assert_eq!(
        result.content,
        Some(serde_json::json!({"from": "delegate"})),
        "the delegate's content must reach the server unchanged"
    );

    let _ = client.cancel().await;
    let _ = server.cancel().await;
}

fn task_status_notification(task_id: &str) -> rmcp::model::ServerNotification {
    let task = rmcp::model::Task::new(
        task_id.to_owned(),
        rmcp::model::TaskStatus::Working,
        "2026-01-01T00:00:00Z".to_owned(),
        "2026-01-01T00:00:00Z".to_owned(),
    );
    let detailed = rmcp::model::DetailedTask::new(task, rmcp::model::TaskPayload::Working);
    rmcp::model::TaskStatusNotification::new(rmcp::model::TaskStatusNotificationParams::new(
        detailed,
    ))
    .into()
}

// Why: task-status and progress are separate notification streams. Feeding a
// task update into the progress dispatcher would surface a phantom progress
// event to whichever caller happened to be awaiting that token, and the
// handler must also not treat an unrecognised notification as fatal — the
// session has to survive it.
#[tokio::test]
async fn a_task_status_notification_is_absorbed_without_disturbing_progress_delivery() {
    let handler = McpClientHandler::new(client_info());
    let dispatcher = handler.progress_dispatcher().clone();
    let (client, server) = connected(handler).await;

    let token = ProgressToken(rmcp::model::NumberOrString::String(Arc::from("job-9")));
    let mut subscriber = dispatcher.subscribe(token.clone()).await;

    server
        .peer()
        .send_notification(task_status_notification("task-9"))
        .await
        .expect("the client accepts a task status notification");

    let leaked =
        tokio::time::timeout(std::time::Duration::from_millis(300), subscriber.next()).await;
    assert!(
        leaked.is_err(),
        "a task update must not be delivered as progress"
    );

    let callback = create_progress_callback(token.clone(), server.peer().clone());
    callback(0.75, None, None).await;

    let received = tokio::time::timeout(std::time::Duration::from_secs(5), subscriber.next())
        .await
        .expect("the session still delivers progress after a task notification")
        .expect("subscriber stream yields");
    assert_eq!(received.progress_token, token);
    assert!((received.progress - 0.75).abs() < f64::EPSILON);

    drop(subscriber);
    let _ = client.cancel().await;
    let _ = server.cancel().await;
}
