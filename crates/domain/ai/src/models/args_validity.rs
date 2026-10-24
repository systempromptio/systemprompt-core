//! Whether a model-emitted tool call's arguments satisfy the input schema the
//! tool was offered with.
//!
//! The verdict is a measurement recorded on `ai_request_tool_calls`; it never
//! changes how a request is served.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The persisted `args_validity` value of one tool call.
///
/// `SchemaUnusable` means the offered schema failed to compile, so the call
/// could not be judged; `UnknownTool` means the model named a tool the request
/// never offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgsValidity {
    Valid,
    Invalid,
    UnknownTool,
    Unparseable,
    SchemaUnusable,
}

impl ArgsValidity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::UnknownTool => "unknown_tool",
            Self::Unparseable => "unparseable",
            Self::SchemaUnusable => "schema_unusable",
        }
    }
}

/// A verdict plus, for `Invalid`, the JSON pointer into the arguments of the
/// first failing instance (`""` is the arguments object itself).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgsCheck {
    pub validity: ArgsValidity,
    pub error_path: Option<String>,
}

impl ArgsCheck {
    pub const fn of(validity: ArgsValidity) -> Self {
        Self {
            validity,
            error_path: None,
        }
    }
}
