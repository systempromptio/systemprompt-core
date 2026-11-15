//! `/api/v1/analytics`: event ingestion (single and batch) and the live
//! analytics stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod events;
mod stream;

pub(crate) use events::{record_event, record_events_batch};
pub(crate) use stream::analytics_stream;
