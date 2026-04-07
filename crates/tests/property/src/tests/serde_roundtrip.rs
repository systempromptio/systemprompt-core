use proptest::prelude::*;
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::protocol::A2aJsonRpcRequest;
use systemprompt_models::a2a::*;

use crate::strategies::a2a::*;
use crate::strategies::jsonrpc::*;

macro_rules! serde_roundtrip {
    ($name:ident, $strategy:expr, $type:ty) => {
        proptest! {
            #[test]
            fn $name(value in $strategy) {
                let json = serde_json::to_string(&value).unwrap();
                let back: $type = serde_json::from_str(&json).unwrap();
                prop_assert_eq!(value, back);
            }
        }
    };
}

serde_roundtrip!(task_state_roundtrip, arb_task_state(), TaskState);
serde_roundtrip!(
    protocol_binding_roundtrip,
    arb_protocol_binding(),
    ProtocolBinding
);
serde_roundtrip!(message_role_roundtrip, arb_message_role(), MessageRole);
serde_roundtrip!(text_part_roundtrip, arb_text_part(), TextPart);
serde_roundtrip!(file_part_roundtrip, arb_file_part(), FilePart);
serde_roundtrip!(data_part_roundtrip, arb_data_part(), DataPart);
serde_roundtrip!(part_roundtrip, arb_part(), Part);
serde_roundtrip!(message_roundtrip, arb_message(), Message);
serde_roundtrip!(task_status_roundtrip, arb_task_status(), TaskStatus);
serde_roundtrip!(task_type_roundtrip, arb_task_type(), TaskType);
serde_roundtrip!(
    task_metadata_roundtrip,
    arb_task_metadata(),
    TaskMetadata
);
serde_roundtrip!(
    artifact_metadata_roundtrip,
    arb_artifact_metadata(),
    ArtifactMetadata
);
serde_roundtrip!(artifact_roundtrip, arb_artifact(), Artifact);
serde_roundtrip!(task_roundtrip, arb_task(), Task);
serde_roundtrip!(agent_skill_roundtrip, arb_agent_skill(), AgentSkill);
serde_roundtrip!(
    agent_capabilities_roundtrip,
    arb_agent_capabilities(),
    AgentCapabilities
);
serde_roundtrip!(
    api_key_location_roundtrip,
    arb_api_key_location(),
    ApiKeyLocation
);
serde_roundtrip!(
    security_scheme_roundtrip,
    arb_security_scheme(),
    SecurityScheme
);
serde_roundtrip!(
    agent_interface_roundtrip,
    arb_agent_interface(),
    AgentInterface
);
serde_roundtrip!(agent_card_roundtrip, arb_agent_card(), AgentCard);
serde_roundtrip!(request_id_roundtrip, arb_request_id(), RequestId);
serde_roundtrip!(jsonrpc_error_roundtrip, arb_jsonrpc_error(), JsonRpcError);
serde_roundtrip!(
    jsonrpc_request_roundtrip,
    arb_jsonrpc_request(),
    A2aJsonRpcRequest
);
serde_roundtrip!(
    jsonrpc_response_task_roundtrip,
    arb_jsonrpc_response_task(),
    JsonRpcResponse<Task>
);
serde_roundtrip!(
    jsonrpc_error_response_roundtrip,
    arb_jsonrpc_error_response(),
    JsonRpcResponse<Task>
);
