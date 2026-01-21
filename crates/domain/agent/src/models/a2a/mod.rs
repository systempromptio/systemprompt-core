pub mod jsonrpc;
pub mod protocol;
mod service_status;

pub use systemprompt_models::a2a::{
    AgentAuthentication, AgentCapabilities, AgentCard, AgentCardBuilder, AgentCardSignature,
    AgentExtension, AgentInterface, AgentProvider, AgentSkill, ApiKeyLocation, Artifact,
    ArtifactMetadata, DataPart, FilePart, FileWithBytes, McpServerMetadata, McpToolsParams,
    Message, MessageMetadata, MessageRole, OAuth2Flow, OAuth2Flows, Part, SecurityScheme, Task,
    TaskState, TaskStatus, TextPart, TransportProtocol,
};

pub use protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequest, A2aRequestParams, A2aResponse, MessageSendParams,
    TaskIdParams, TaskQueryParams,
};
pub use service_status::ServiceStatusParams;
