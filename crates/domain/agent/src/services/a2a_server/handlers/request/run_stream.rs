//! The SSE stream that follows an agent run after its `Submitted` task.
//!
//! [`follow_run`] sends the `Submitted` task, then polls
//! `AgentRunLauncher::updates_after` and forwards each status and artifact
//! frame. The stream closes once the run is finished, after a terminal status
//! frame (the run's own, or the launcher's `final_status` when the run ended
//! without one), or as soon as the client disconnects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;
use tokio_util::task::TaskTracker;

use axum::response::sse::Event;
use systemprompt_identifiers::AgentRunId;
use systemprompt_traits::DynAgentRunLauncher;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;

use crate::models::a2a::jsonrpc::NumberOrString;
use crate::models::a2a::{StreamResponse, Task};
use crate::services::a2a_server::streaming::stream_response_event;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub(super) fn follow_run(
    task: Task,
    launcher: DynAgentRunLauncher,
    run_id: Option<AgentRunId>,
    request_id: NumberOrString,
    tracker: &TaskTracker,
) -> ReceiverStream<Event> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    tracker.spawn(async move {
        let submitted = stream_response_event(StreamResponse::Task(task), &request_id);
        if tx.send(submitted).await.is_err() {
            return;
        }
        if let Some(run_id) = run_id {
            forward_updates(&tx, &launcher, &run_id, &request_id).await;
        }
    });
    ReceiverStream::new(rx)
}

async fn forward_updates(
    tx: &Sender<Event>,
    launcher: &DynAgentRunLauncher,
    run_id: &AgentRunId,
    request_id: &NumberOrString,
) {
    let mut after_seq = 0;
    loop {
        let updates = match launcher.updates_after(run_id, after_seq).await {
            Ok(updates) => updates,
            Err(e) => {
                tracing::warn!(run_id = %run_id, error = %e, "Agent run updates unavailable");
                return;
            },
        };
        after_seq = updates.last_seq;
        for value in updates.frames {
            match send_frame(tx, value, request_id).await {
                Sent::Terminal | Sent::Closed => return,
                Sent::Open => {},
            }
        }
        if updates.finished {
            if let Some(value) = updates.final_status {
                send_frame(tx, value, request_id).await;
            }
            return;
        }
        tokio::select! {
            () = tokio::time::sleep(POLL_INTERVAL) => {},
            () = tx.closed() => return,
        }
    }
}

enum Sent {
    Open,
    Terminal,
    Closed,
}

async fn send_frame(
    tx: &Sender<Event>,
    value: serde_json::Value,
    request_id: &NumberOrString,
) -> Sent {
    let response = match serde_json::from_value::<StreamResponse>(value) {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(error = %e, "Skipping malformed agent run stream frame");
            return Sent::Open;
        },
    };
    let terminal = matches!(
        &response,
        StreamResponse::StatusUpdate(event) if event.status.state.is_terminal()
    );
    if tx
        .send(stream_response_event(response, request_id))
        .await
        .is_err()
    {
        return Sent::Closed;
    }
    if terminal { Sent::Terminal } else { Sent::Open }
}
