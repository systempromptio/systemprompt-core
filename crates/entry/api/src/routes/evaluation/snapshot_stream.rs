//! Bounded authenticated snapshot streams use durable generations and core
//! connection guards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::snapshot_generation::Generation;
use crate::routes::stream::StreamWithGuard;
use axum::Extension;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::panic::AssertUnwindSafe;
use std::sync::LazyLock;
use systemprompt_events::{
    Broadcaster, ConnectionGuard, GenericBroadcaster, ToSse, standard_keep_alive,
};
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub(super) struct FeedbackWake {
    generation: Generation,
    resync: bool,
}
impl ToSse for FeedbackWake {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        Ok(Event::default()
            .id(self.generation.token())
            .event(if self.resync { "resync" } else { "snapshot" })
            .data(serde_json::to_string(self)?))
    }
}
pub(super) static CONNECTIONS: LazyLock<GenericBroadcaster<FeedbackWake>> =
    LazyLock::new(GenericBroadcaster::new);
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Resume {
    after: Option<String>,
}

pub(super) async fn stream(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    headers: HeaderMap,
    Query(query): Query<Resume>,
) -> axum::response::Response {
    let resume = match headers
        .get("last-event-id")
        .map(|value| value.to_str())
        .transpose()
    {
        Ok(value) => value.map(str::to_owned).or(query.after),
        Err(_error) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let resume = match resume.as_deref().map(Generation::parse) {
        Some(None) => return StatusCode::BAD_REQUEST.into_response(),
        Some(Some(value)) => Some(value),
        None => None,
    };
    let id = ConnectionId::generate();
    let user = actor.user_id().clone();
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(8);
    if !CONNECTIONS.register(&user, &id, tx.clone()).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let guard = ConnectionGuard::new(&CONNECTIONS, user, id.clone());
    let notifications = ctx.snapshot_wakeup().subscribe(ctx.db_pool()).await;
    let forwarder = tokio::spawn(async move {
        if AssertUnwindSafe(forward(ctx, tx, resume, notifications))
            .catch_unwind()
            .await
            .is_err()
        {
            tracing::error!(connection_id = %id, "Snapshot stream forwarder panicked");
        }
    });
    Sse::new(OwnedStream {
        inner: StreamWithGuard::new(ReceiverStream::new(rx), guard),
        forwarder,
    })
    .keep_alive(standard_keep_alive())
    .into_response()
}

// Why: the forwarder is bounded by the receiver's lifetime, but a stream that
// is dropped mid-poll must not leave it running until its next tick notices.
struct OwnedStream {
    inner: StreamWithGuard<FeedbackWake>,
    forwarder: JoinHandle<()>,
}

impl Drop for OwnedStream {
    fn drop(&mut self) {
        self.forwarder.abort();
    }
}

impl futures_util::Stream for OwnedStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}
async fn forward(
    ctx: AppContext,
    tx: mpsc::Sender<Result<Event, Infallible>>,
    mut previous: Option<Generation>,
    mut notifications: tokio::sync::watch::Receiver<u64>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut initial = true;
    let mut failing = false;
    loop {
        tokio::select! {()=tx.closed()=>break,_tick=interval.tick()=>{},changed=notifications.changed()=>{if changed.is_err(){break;}}}
        let read = tokio::select! {()=tx.closed()=>break,result=read_generation(&ctx)=>result};
        let generation = match read {
            Ok(generation) => {
                if failing {
                    tracing::info!("Snapshot stream generation reads recovered");
                    failing = false;
                }
                generation
            },
            Err(error) => {
                if !failing {
                    tracing::warn!(%error, "Snapshot stream generation read failed; retrying");
                    failing = true;
                }
                continue;
            },
        };
        if initial || previous != Some(generation) {
            let event = FeedbackWake {
                generation,
                resync: previous.is_none_or(|old| generation.requires_resync(old))
                    || (initial && previous != Some(generation)),
            };
            let Ok(event) = event.to_sse() else {
                break;
            };
            tokio::select! {()=tx.closed()=>break,result=tx.send(Ok(event))=>{if result.is_err(){break;}}}
            previous = Some(generation);
            initial = false;
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum GenerationReadError {
    #[error(transparent)]
    Snapshots(#[from] systemprompt_analytics::AnalyticsError),
    #[error(transparent)]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),
}

async fn read_generation(ctx: &AppContext) -> Result<Generation, GenerationReadError> {
    let owner = ctx.system_admin().id();
    let health = ctx.feedback_snapshots_repository().health(owner).await?;
    let inventory = ctx.managed_repository().inventory_status(owner).await?;
    let installations = ctx
        .managed_repository()
        .installation_coverage_status(owner)
        .await?;
    Ok(Generation {
        snapshots: health.generation,
        inventory: inventory.generation,
        installations: installations.generation,
    })
}
