//! Execution models — request context propagation, per-run execution
//! steps, and the shared context carried through an agent run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod context;
pub mod shared_context;
pub mod step;

pub use context::{CallSource, ContextExtractionError, ContextIdSource, RequestContext};
pub use shared_context::SharedRequestContext;
pub use step::{
    ExecutionStep, PlannedTool, StepContent, StepDetail, StepId, StepStatus, StepType, TrackedStep,
};
