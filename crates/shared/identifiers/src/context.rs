//! Execution-context identifier — UUID v4 only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;
use crate::{EvalRunId, GatewayConversationId, SessionId, TaskId};

crate::define_id!(ContextId, validated, schema, validate_uuid_v4);

fn validate_uuid_v4(s: &str) -> Result<(), IdValidationError> {
    uuid::Uuid::parse_str(s).map_err(|e| IdValidationError::invalid("ContextId", e.to_string()))?;
    Ok(())
}

const GATEWAY_CONVERSATION_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x993f_3f2c_f4d9_463b_853a_d3f0_3e19_0898);

const MESSAGING_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x6b1d_2a7e_9c84_4f31_b5e0_71a2_4d8c_3f06);

const SESSION_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x4c1e_8b02_7a63_4d51_9f2c_0e58_a7d4_31bb);

const EVALUATION_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x7f3a_c2d1_5b09_4e87_a6f4_2c91_d05e_88a3);

const CLI_PROBE_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x2d84_9f60_1c3b_4a72_8e15_b7d0_63f9_a541);

const MCP_VALIDATION_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x91b6_04ce_7d2f_4380_b8a9_5e16_c74d_2f08);

const TASK_NAMESPACE: uuid::Uuid = uuid::Uuid::from_u128(0x5ae0_37b9_8f14_4c26_9d7b_e842_06c1_fd35);

const LEGACY_CONTEXT_UUID: &str = "00000000-0000-0000-0000-4c4547414359";

impl ContextId {
    pub fn generate() -> Self {
        Self::new_unchecked(uuid::Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn derived_from_gateway_conversation(gw: &GatewayConversationId) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&GATEWAY_CONVERSATION_NAMESPACE, gw.as_str().as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn derived_from_messaging(platform: &str, org: &str, channel: &str) -> Self {
        let key = format!("{platform}:{org}:{channel}");
        Self::new_unchecked(uuid::Uuid::new_v5(&MESSAGING_NAMESPACE, key.as_bytes()).to_string())
    }

    #[must_use]
    pub fn derived_from_session(session_id: &SessionId) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&SESSION_NAMESPACE, session_id.as_str().as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn derived_from_evaluation_run(run_id: &EvalRunId) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&EVALUATION_NAMESPACE, run_id.as_str().as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn derived_from_cli_probe(server_name: &str) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&CLI_PROBE_NAMESPACE, server_name.as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn derived_from_mcp_validation(service_name: &str) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&MCP_VALIDATION_NAMESPACE, service_name.as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn derived_from_task(task_id: &TaskId) -> Self {
        Self::new_unchecked(
            uuid::Uuid::new_v5(&TASK_NAMESPACE, task_id.as_str().as_bytes()).to_string(),
        )
    }

    #[must_use]
    pub fn legacy() -> Self {
        Self::new_unchecked(LEGACY_CONTEXT_UUID)
    }
}
