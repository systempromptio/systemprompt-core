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
    HandleCompleteParams, HandleErrorParams, handle_complete, handle_error,
};
pub(super) use text::TextStreamState;
