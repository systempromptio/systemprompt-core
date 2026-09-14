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
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::LazyLock;
use systemprompt_events::{
    Broadcaster, ConnectionGuard, GenericBroadcaster, ToSse, standard_keep_alive,
};
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Deserialize)]
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
    let guard = ConnectionGuard::new(&CONNECTIONS, user, id);
    let notifications = super::snapshot_wakeup::subscribe(&ctx).await;
    tokio::spawn(forward(ctx, tx, resume, notifications));
    Sse::new(StreamWithGuard::new(ReceiverStream::new(rx), guard))
        .keep_alive(standard_keep_alive())
        .into_response()
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
    loop {
        tokio::select! {()=tx.closed()=>break,_tick=interval.tick()=>{},changed=notifications.changed()=>{if changed.is_err(){break;}}}
        let health = tokio::select! {()=tx.closed()=>break,result=ctx.feedback_snapshots_repository().health(ctx.system_admin().id())=>result};
        let Ok(health) = health else {
            continue;
        };
        let coverage = tokio::select! {()=tx.closed()=>break,result=async{let inventory=ctx.managed_repository().inventory_status(ctx.system_admin().id()).await?;let installations=ctx.managed_repository().installation_coverage_status(ctx.system_admin().id()).await?;Ok::<_,systemprompt_marketplace::managed::ManagedError>((inventory,installations))}=>result};
        let Ok((inventory, installations)) = coverage else {
            continue;
        };
        let generation = Generation {
            snapshots: health.generation,
            inventory: inventory.generation,
            installations: installations.generation,
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
