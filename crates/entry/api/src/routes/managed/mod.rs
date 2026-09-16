//! Managed-resource administration.
//!
//! Sources, revisions, inventory, publications, analytics snapshots and the
//! device-credential consumer surface. Authentication is supplied by the core
//! admin middleware; actor identity is never accepted from JSON.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod collections;
pub mod consumer;
pub mod contract;
mod error;
mod inventory;
mod operation_handlers;
pub mod operations;
pub mod origin;
mod publications;
mod resources;
mod snapshot_generation;
mod snapshot_stream;
mod snapshots;
pub mod state;

use axum::Router;
use axum::routing::get;

use self::state::ManagedState;

pub fn router() -> Router<ManagedState> {
    Router::new()
        .route("/openapi.json", get(contract::openapi::serve))
        .merge(resources::router())
        .merge(inventory::router())
        .merge(operations::router())
        .merge(publications::router())
        .merge(snapshots::router())
        .merge(consumer::admin_router())
}
