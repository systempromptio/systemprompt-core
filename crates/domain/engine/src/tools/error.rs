//! The error a tool call ends with when it produces no output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;

/// A boxed cause carried by [`ToolError::Failed`] and [`ToolError::Malformed`].
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Why a tool call did not produce output. The engine shows the Display text to
/// the model.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("permission denied: {reason}")]
    Denied { reason: String },
    #[error("the user rejected this action")]
    Rejected,
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("invalid input: {}", malformed_text(.context, .source))]
    Malformed {
        context: String,
        #[source]
        source: BoxError,
    },
    #[error("the tool call was aborted")]
    Aborted,
    #[error("{message}")]
    Failed {
        message: String,
        #[source]
        source: Option<BoxError>,
    },
}

impl ToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn malformed(context: impl Into<String>, source: impl Into<BoxError>) -> Self {
        Self::Malformed {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
            source: None,
        }
    }

    pub fn caused(message: impl Into<String>, source: impl Into<BoxError>) -> Self {
        Self::Failed {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    pub fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::caused(message, source)
    }
}

fn malformed_text(context: &str, source: &BoxError) -> String {
    if context.is_empty() {
        source.to_string()
    } else {
        format!("{context}: {source}")
    }
}
