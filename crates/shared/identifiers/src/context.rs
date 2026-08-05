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
        Self::new(uuid::Uuid::new_v4().to_string())
    }

    /// Mint a deterministic `ContextId` from a `GatewayConversationId`.
    ///
    /// Same gateway-conversation id always produces the same `ContextId`, so
    /// the gateway boundary can satisfy the "every conversation has a UUID
    /// `ContextId`" data-integrity invariant without trusting the upstream
    /// LLM client's `x-context-id` header (which carries client-specific
    /// non-UUID identifiers).
    #[must_use]
    pub fn derived_from_gateway_conversation(gw: &GatewayConversationId) -> Self {
        Self::new(
            uuid::Uuid::new_v5(&GATEWAY_CONVERSATION_NAMESPACE, gw.as_str().as_bytes()).to_string(),
        )
    }

    /// Mint a deterministic `ContextId` for a chat-platform conversation.
    ///
    /// The same `(platform, org, channel)` triple — e.g.
    /// `("slack", workspace_id, channel_id)` or
    /// `("teams", tenant_id, conversation_id)` — always produces the same
    /// `ContextId`, so the messaging dispatch boundary satisfies the "every
    /// conversation has a UUID `ContextId`" invariant without a channel→context
    /// mapping table.
    #[must_use]
    pub fn derived_from_messaging(platform: &str, org: &str, channel: &str) -> Self {
        let key = format!("{platform}:{org}:{channel}");
        Self::new(uuid::Uuid::new_v5(&MESSAGING_NAMESPACE, key.as_bytes()).to_string())
    }

    /// Mint a deterministic `ContextId` from a `SessionId`.
    ///
    /// Same session always produces the same `ContextId`, so session-scoped
    /// boundaries with no conversation of their own (anonymous sessions,
    /// hook-triggered inference such as session summaries, header fallbacks)
    /// satisfy the "every AI request belongs to exactly one real context"
    /// invariant without a session→context mapping table.
    #[must_use]
    pub fn derived_from_session(session_id: &SessionId) -> Self {
        Self::new(
            uuid::Uuid::new_v5(&SESSION_NAMESPACE, session_id.as_str().as_bytes()).to_string(),
        )
    }

    /// Mint a deterministic `ContextId` from an evaluation run id.
    ///
    /// Same run always produces the same `ContextId`, so every judge and
    /// replay request of one evaluation run lands in one context.
    #[must_use]
    pub fn derived_from_evaluation_run(run_id: &EvalRunId) -> Self {
        Self::new(uuid::Uuid::new_v5(&EVALUATION_NAMESPACE, run_id.as_str().as_bytes()).to_string())
    }

    /// Mint a deterministic `ContextId` for a CLI probe of an MCP server.
    ///
    /// Same server name always produces the same `ContextId`, so repeated
    /// diagnostic probes of one server share one context.
    #[must_use]
    pub fn derived_from_cli_probe(server_name: &str) -> Self {
        Self::new(uuid::Uuid::new_v5(&CLI_PROBE_NAMESPACE, server_name.as_bytes()).to_string())
    }

    /// Mint a deterministic `ContextId` for MCP service validation.
    ///
    /// Same service name always produces the same `ContextId`, so repeated
    /// validation passes of one service share one context.
    #[must_use]
    pub fn derived_from_mcp_validation(service_name: &str) -> Self {
        Self::new(
            uuid::Uuid::new_v5(&MCP_VALIDATION_NAMESPACE, service_name.as_bytes()).to_string(),
        )
    }

    /// Mint a deterministic `ContextId` from a `TaskId`.
    ///
    /// Fallback for task-scoped boundaries when the task's real context row
    /// cannot be resolved; same task always produces the same `ContextId`.
    #[must_use]
    pub fn derived_from_task(task_id: &TaskId) -> Self {
        Self::new(uuid::Uuid::new_v5(&TASK_NAMESPACE, task_id.as_str().as_bytes()).to_string())
    }

    /// The fixed context that absorbs pre-invariant historical rows.
    ///
    /// Rows written before context ids became mandatory were backfilled to
    /// this constant; read paths that meet an unparseable stored id also
    /// resolve here rather than fabricating a fresh id.
    #[must_use]
    pub fn legacy() -> Self {
        Self::new(LEGACY_CONTEXT_UUID)
    }
}
