//! Streaming surface — SSE channel construction, lifecycle event broadcasting,
//! and the per-task event loop that fans the model's stream out to A2A,
//! AG-UI webhooks, and SSE clients.

mod agent_loader;
pub mod broadcast;
mod event_loop;
mod event_loop_lifecycle;
mod handlers;
mod initialization;
mod initialization_steps;
mod messages;
mod types;
pub mod webhook_client;

pub use broadcast::{broadcast_artifact_created, broadcast_task_completed};
pub use event_loop::ProcessEventsParams;
pub use event_loop_lifecycle::{
    EmitRunStartedParams, emit_run_started, handle_stream_creation_error,
};
pub use messages::{CreateSseStreamParams, create_sse_stream};
pub use types::{PersistTaskInput, StreamContext, StreamInput, StreamSetupResult};
