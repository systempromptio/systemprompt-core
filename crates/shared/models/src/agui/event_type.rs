use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgUiEventType {
    RunStarted,
    RunFinished,
    RunError,
    StepStarted,
    StepFinished,
    TextMessageStart,
    TextMessageContent,
    TextMessageEnd,
    ToolCallStart,
    ToolCallArgs,
    ToolCallEnd,
    ToolCallResult,
    StateSnapshot,
    StateDelta,
    MessagesSnapshot,
    Custom,
}

impl AgUiEventType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RunStarted => "RUN_STARTED",
            Self::RunFinished => "RUN_FINISHED",
            Self::RunError => "RUN_ERROR",
            Self::StepStarted => "STEP_STARTED",
            Self::StepFinished => "STEP_FINISHED",
            Self::TextMessageStart => "TEXT_MESSAGE_START",
            Self::TextMessageContent => "TEXT_MESSAGE_CONTENT",
            Self::TextMessageEnd => "TEXT_MESSAGE_END",
            Self::ToolCallStart => "TOOL_CALL_START",
            Self::ToolCallArgs => "TOOL_CALL_ARGS",
            Self::ToolCallEnd => "TOOL_CALL_END",
            Self::ToolCallResult => "TOOL_CALL_RESULT",
            Self::StateSnapshot => "STATE_SNAPSHOT",
            Self::StateDelta => "STATE_DELTA",
            Self::MessagesSnapshot => "MESSAGES_SNAPSHOT",
            Self::Custom => "CUSTOM",
        }
    }
}
