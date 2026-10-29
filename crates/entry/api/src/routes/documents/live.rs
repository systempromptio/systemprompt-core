//! `GET /api/v1/documents/{id}/live`: the document-scoped live channel, and
//! `POST /api/v1/documents/{id}/collab/awareness`.
//!
//! The channel is server-sent events gated on view access. It opens with
//! `document.live.ready` and the awareness states still alive, then carries
//! every event published on the document's topic: collab updates, awareness,
//! comments, revisions, locks and presence. The channel subscribes before
//! it reads the session, and the ready event carries the last sequence
//! number the session had logged then. Each collab update's SSE id is
//! `<session>:<seq>` where `seq` is the highest number below which the
//! channel delivered every update, so an update relayed out of order is
//! never skipped by a resume. A client that reconnects with that
//! `Last-Event-ID` header (or `?last_event_id=`) first receives the updates
//! after it, or a `document.live.resync` asking it to re-join when the log
//! was compacted, the session changed, the channel lagged or the
//! cross-replica relay lost events. Access is re-checked every 30 s; a
//! revoked caller receives `document.live.closed` and the stream ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use systemprompt_events::{RELAY_LOST_EVENT, TopicEvent, standard_keep_alive};
use systemprompt_files::documents::agents::act_as;
use systemprompt_files::documents::events::live_topic;
use systemprompt_files::documents::live::{
    LiveCursor, LiveDelivery, LiveOpen, LiveReplay, ResumeToken,
};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::{
    AwarenessState, DocumentAwarenessEvent, DocumentCollabUpdateEvent, DocumentLiveClosedEvent,
    DocumentLiveReadyEvent, DocumentLiveResyncEvent, LiveResyncReason,
};
use tokio::sync::broadcast::error::RecvError;

use super::DocumentsState;
use super::caller::{Caller, caller};
use super::error::DocumentsResult;
use super::items::document_id;

const ACCESS_RECHECK: Duration = Duration::from_secs(30);

enum Step {
    Received(Result<TopicEvent, RecvError>),
    Recheck,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct LiveQuery {
    last_event_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct AwarenessJson {
    published: bool,
    expires_at: chrono::DateTime<chrono::Utc>,
}

pub(super) async fn live(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<LiveQuery>,
    headers: HeaderMap,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let document = document_id(&id)?;
    let topic = state
        .service
        .events()
        .router()
        .broadcasters()
        .topics
        .subscribe_resumable(&live_topic(&document));
    let open = state
        .service
        .live_open(&who.user, &document, &who.trace)
        .await?;
    let resume = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or(query.last_event_id)
        .as_deref()
        .and_then(ResumeToken::parse);
    let replay = match &resume {
        Some(token) => Some(state.service.live_replay(&open, token).await?),
        None => None,
    };
    let delivered = LiveCursor::start(&open, resume.as_ref(), replay.as_ref());
    let stream = channel(state, who, open, (replay, delivered), topic);
    Ok(Sse::new(stream)
        .keep_alive(standard_keep_alive())
        .into_response())
}

pub(super) async fn awareness(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<AwarenessState>,
) -> DocumentsResult<Json<AwarenessJson>> {
    let who = caller(&state, &req_ctx).await?;
    let outcome = state
        .service
        .publish_awareness(&who.user, &document_id(&id)?, body, &who.trace)
        .await?;
    Ok(Json(AwarenessJson {
        published: outcome.published,
        expires_at: outcome.event.expires_at,
    }))
}

fn channel(
    state: DocumentsState,
    who: Caller,
    open: LiveOpen,
    (replay, mut delivered): (Option<LiveReplay>, LiveCursor),
    mut topic: tokio::sync::broadcast::Receiver<TopicEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let document = open.document.id.clone();
        let mut replayed = 0;
        let mut resync = None;
        let mut updates = Vec::new();
        match replay {
            Some(LiveReplay::Updates(rows)) => updates = rows,
            Some(LiveReplay::Resync(reason)) => resync = Some(reason),
            None => {},
        }
        yield Ok(frame(DocumentLiveReadyEvent::NAME, None, &DocumentLiveReadyEvent {
            document_id: document.clone(),
            edit_session_id: open.edit_session.clone(),
            seq: open.seq,
            replayed: updates.len(),
        }));
        if let Some(reason) = resync {
            yield Ok(resync_frame(&open, reason));
        }
        for update in updates {
            replayed += 1;
            let id = delivered.replayed(&update.edit_session_id, update.seq);
            yield Ok(frame(DocumentCollabUpdateEvent::NAME, Some(id), &update));
        }
        tracing::debug!(document_id = %document, replayed, "Live channel opened");
        for aware in &open.awareness {
            yield Ok(frame(DocumentAwarenessEvent::NAME, None, aware));
        }
        let mut recheck = tokio::time::interval(ACCESS_RECHECK);
        recheck.tick().await;
        loop {
            let step = tokio::select! {
                received = topic.recv() => Step::Received(received),
                _ = recheck.tick() => Step::Recheck,
            };
            match step {
                Step::Received(Ok(event)) if event.name == RELAY_LOST_EVENT => {
                    yield Ok(resync_frame(&open, LiveResyncReason::RelayLost));
                },
                Step::Received(Ok(event)) => {
                    if let LiveDelivery::Deliver(id) = delivered.live(&event) {
                        yield Ok(frame(&event.name, id, &event.value));
                    }
                },
                Step::Received(Err(RecvError::Lagged(_))) => {
                    yield Ok(resync_frame(&open, LiveResyncReason::Lagged));
                },
                Step::Received(Err(RecvError::Closed)) => break,
                Step::Recheck => {
                    let allowed = act_as(
                        who.actor.clone(),
                        state.service.live_allowed(&who.user, &document, &who.trace),
                    )
                    .await;
                    if matches!(allowed, Ok(false)) {
                        yield Ok(frame(DocumentLiveClosedEvent::NAME, None, &DocumentLiveClosedEvent {
                            document_id: document.clone(),
                        }));
                        break;
                    }
                },
            }
        }
    }
}

fn resync_frame(open: &LiveOpen, reason: LiveResyncReason) -> Event {
    frame(
        DocumentLiveResyncEvent::NAME,
        None,
        &DocumentLiveResyncEvent {
            document_id: open.document.id.clone(),
            reason,
        },
    )
}

fn frame<T: Serialize>(name: &str, id: Option<String>, payload: &T) -> Event {
    let event = Event::default().event(name);
    let event = match id {
        Some(id) => event.id(id),
        None => event,
    };
    event.json_data(payload).unwrap_or_else(|error| {
        tracing::warn!(event = name, error = %error, "Live event could not be serialized");
        Event::default().event(name).data("{}")
    })
}
