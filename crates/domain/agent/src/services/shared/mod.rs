//! The service-local error type shared by the agent service layer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;

pub use error::{AgentServiceError, Result};
