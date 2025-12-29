pub mod agui {
    pub const RUN_STARTED: &str = "RUN_STARTED";
    pub const RUN_FINISHED: &str = "RUN_FINISHED";
    pub const RUN_ERROR: &str = "RUN_ERROR";
    pub const STEP_STARTED: &str = "STEP_STARTED";
    pub const STEP_FINISHED: &str = "STEP_FINISHED";
    pub const TEXT_MESSAGE_START: &str = "TEXT_MESSAGE_START";
    pub const TEXT_MESSAGE_CONTENT: &str = "TEXT_MESSAGE_CONTENT";
    pub const TEXT_MESSAGE_END: &str = "TEXT_MESSAGE_END";
    pub const TOOL_CALL_START: &str = "TOOL_CALL_START";
    pub const TOOL_CALL_ARGS: &str = "TOOL_CALL_ARGS";
    pub const TOOL_CALL_END: &str = "TOOL_CALL_END";
    pub const TOOL_CALL_RESULT: &str = "TOOL_CALL_RESULT";
    pub const STATE_SNAPSHOT: &str = "STATE_SNAPSHOT";
    pub const STATE_DELTA: &str = "STATE_DELTA";
    pub const MESSAGES_SNAPSHOT: &str = "MESSAGES_SNAPSHOT";
    pub const CUSTOM: &str = "CUSTOM";
}

pub mod a2a {
    pub const TASK_SUBMITTED: &str = "TASK_SUBMITTED";
    pub const TASK_STATUS_UPDATE: &str = "TASK_STATUS_UPDATE";
    pub const ARTIFACT_CREATED: &str = "ARTIFACT_CREATED";
    pub const ARTIFACT_UPDATED: &str = "ARTIFACT_UPDATED";
    pub const AGENT_MESSAGE: &str = "AGENT_MESSAGE";
    pub const INPUT_REQUIRED: &str = "INPUT_REQUIRED";
    pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
    pub const JSON_RPC_RESPONSE: &str = "JSON_RPC_RESPONSE";
    pub const JSON_RPC_ERROR: &str = "JSON_RPC_ERROR";
}

pub mod system {
    pub const CONTEXT_CREATED: &str = "CONTEXT_CREATED";
    pub const CONTEXT_UPDATED: &str = "CONTEXT_UPDATED";
    pub const CONTEXT_DELETED: &str = "CONTEXT_DELETED";
    pub const CONTEXTS_SNAPSHOT: &str = "CONTEXTS_SNAPSHOT";
    pub const CONNECTED: &str = "CONNECTED";
    pub const HEARTBEAT: &str = "HEARTBEAT";
}

pub mod jsonrpc {
    pub const MESSAGE_SEND: &str = "message/send";
    pub const MESSAGE_STREAM: &str = "message/stream";
    pub const TASKS_GET: &str = "tasks/get";
    pub const TASKS_CANCEL: &str = "tasks/cancel";
    pub const TASKS_RESUBSCRIBE: &str = "tasks/resubscribe";
    pub const AGENT_GET_CARD: &str = "agent/getAuthenticatedExtendedCard";
    pub const PUSH_CONFIG_SET: &str = "tasks/pushNotificationConfig/set";
    pub const PUSH_CONFIG_GET: &str = "tasks/pushNotificationConfig/get";
    pub const PUSH_CONFIG_LIST: &str = "tasks/pushNotificationConfig/list";
    pub const PUSH_CONFIG_DELETE: &str = "tasks/pushNotificationConfig/delete";
}

pub mod protocol {
    pub const AGUI: &str = "agui";
    pub const A2A: &str = "a2a";
    pub const SYSTEM: &str = "system";
}
