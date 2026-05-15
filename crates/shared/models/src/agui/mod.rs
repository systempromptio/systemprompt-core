//! AG-UI streaming event protocol shapes.
//!
//! Wire types for the AG-UI event stream: the [`AgUiEvent`] envelope and
//! its [`AgUiEventBuilder`], the [`AgUiEventType`] discriminant, JSON
//! Patch state-delta operations, and the per-event payload structs
//! (run lifecycle, text messages, tool calls, state snapshots).

mod event_type;
mod events;
mod json_patch;
mod payloads;

pub use event_type::AgUiEventType;
pub use events::{AgUiEvent, AgUiEventBuilder};
pub use json_patch::{JsonPatchOperation, StateDeltaBuilder};
pub use payloads::{
    ArtifactCustomPayload, CustomPayload, ExecutionStepCustomPayload, GenericCustomPayload,
    MessageRole, MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload, RunStartedPayload,
    SkillLoadedCustomPayload, StateDeltaPayload, StateSnapshotPayload, StepFinishedPayload,
    StepStartedPayload, TextMessageContentPayload, TextMessageEndPayload, TextMessageStartPayload,
    ToolCallArgsPayload, ToolCallEndPayload, ToolCallResultPayload, ToolCallStartPayload,
};
