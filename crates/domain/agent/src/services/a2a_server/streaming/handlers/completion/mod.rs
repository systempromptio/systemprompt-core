//! Terminal stream-event handlers: task completion and failure.
//!
//! [`handle_complete`] persists the finished task and broadcasts the success
//! events; [`record_failure`] / [`announce_failure`] and
//! [`announce_cancelled`] cover the failure and cancellation paths. Every
//! terminal frame goes through the event loop's single status emitter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod complete;
mod error;
mod success;

pub(in crate::services::a2a_server::streaming) use complete::{
    HandleCompleteParams, handle_complete,
};

pub(in crate::services::a2a_server::streaming) use error::{
    AnnounceFailureParams, announce_cancelled, announce_failure, record_failure,
};

pub(super) use super::super::event_loop_lifecycle::send_a2a_status_event;
