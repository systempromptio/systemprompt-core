//! Per-event handlers for the A2A streaming pipeline.
//!
//! Routes incoming stream events to their handlers: `completion` for terminal
//! completion and error events, `text` for incremental text accumulation via
//! [`TextStreamState`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod completion;
mod text;

pub(super) use completion::{
    AnnounceFailureParams, HandleCompleteParams, announce_cancelled, announce_failure,
    handle_complete, record_failure,
};
pub(super) use text::TextStreamState;
