pub mod headers;
pub mod payload;

pub use headers::HeaderSource;
pub use payload::PayloadSource;
pub use systemprompt_models::execution::{ContextIdSource, TASK_BASED_CONTEXT_MARKER};
