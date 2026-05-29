//! Per-event handlers for the A2A streaming pipeline.
//!
//! Routes incoming stream events to their handlers: `completion` for terminal
//! completion and error events, `text` for incremental text accumulation via
//! [`TextStreamState`].

mod completion;
mod text;

pub(super) use completion::{
    HandleCompleteParams, HandleErrorParams, handle_complete, handle_error,
};
pub(super) use text::TextStreamState;
