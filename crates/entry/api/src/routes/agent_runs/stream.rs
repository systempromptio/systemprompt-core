//! `GET /api/v1/agent-runs/{id}/events/stream`: the run's events as SSE.
//!
//! The stream reads the event table from the last `seq` it sent, so a
//! reconnect with `Last-Event-ID` resumes without loss, and ends once the
//! run is terminal and every event has been sent. Between reads it waits for
//! this run's commit notification, with [`BACKSTOP`] as the re-read bound
//! when a notification is lost.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use std::convert::Infallible;
use systemprompt_agent_runs::RunStatus;
use systemprompt_agent_runs::notify::BACKSTOP;
use systemprompt_models::RequestContext;
use tokio::sync::broadcast::error::RecvError;

use super::AgentRunsState;
use super::handlers::{EVENT_PAGE, owned_run};
use crate::error::ApiHttpError;

pub(super) async fn events_stream(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<AgentRunsState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiHttpError> {
    let row = owned_run(&state, &req_ctx, &id).await?;
    let after = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(-1);
    let mut wake = state.notifier.subscribe();
    let stream = async_stream::stream! {
        let mut after = after;
        loop {
            let page = match state.runs.list_events_after(&row.id, after, EVENT_PAGE).await {
                Ok(page) => page,
                Err(error) => {
                    tracing::error!(run_id = %row.id, %error, "agent run event stream failed");
                    yield Ok(Event::default().event("error").data("event read failed"));
                    return;
                }
            };
            let drained = page.len() < usize::try_from(EVENT_PAGE).unwrap_or(usize::MAX);
            for event in page {
                after = event.seq;
                match Event::default().id(event.seq.to_string()).event("run_event").json_data(&event) {
                    Ok(sse) => yield Ok(sse),
                    Err(error) => tracing::warn!(run_id = %row.id, %error, "event not serialisable"),
                }
            }
            if drained {
                let terminal = match state.runs.find_run(&row.id).await {
                    Ok(Some(current)) => current
                        .status
                        .parse::<RunStatus>()
                        .is_ok_and(RunStatus::is_terminal),
                    Ok(None) => true,
                    Err(error) => {
                        tracing::error!(run_id = %row.id, %error, "agent run status read failed");
                        true
                    }
                };
                if terminal {
                    yield Ok(Event::default().event("end").data(row.id.as_str()));
                    return;
                }
                let deadline = tokio::time::sleep(BACKSTOP);
                tokio::pin!(deadline);
                loop {
                    tokio::select! {
                        () = &mut deadline => break,
                        received = wake.recv() => match received {
                            Ok(id) if id == row.id => break,
                            Ok(_) => {},
                            Err(RecvError::Lagged(_)) => break,
                            Err(RecvError::Closed) => {
                                deadline.as_mut().await;
                                break;
                            }
                        },
                    }
                }
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
