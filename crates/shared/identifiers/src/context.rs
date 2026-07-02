//! Execution-context identifier — UUID v4 only.

use crate::GatewayConversationId;
use crate::error::IdValidationError;

crate::define_id!(ContextId, validated, schema, validate_uuid_v4);

fn validate_uuid_v4(s: &str) -> Result<(), IdValidationError> {
    uuid::Uuid::parse_str(s).map_err(|e| IdValidationError::invalid("ContextId", e.to_string()))?;
    Ok(())
}

// Why: UUID v5 namespace for deriving a stable `ContextId` from a
// `GatewayConversationId`. Hardcoded so derivations match across processes
// and rebuilds; rotating it would orphan every prior gateway audit row.
const GATEWAY_CONVERSATION_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x993f_3f2c_f4d9_463b_853a_d3f0_3e19_0898);

// Why: UUID v5 namespace for deriving a stable `ContextId` from a messaging
// surface's (platform, org, channel) triple. Hardcoded so derivations match
// across processes and rebuilds; rotating it would orphan every prior Slack /
// Teams conversation's audit history.
const MESSAGING_NAMESPACE: uuid::Uuid =
    uuid::Uuid::from_u128(0x6b1d_2a7e_9c84_4f31_b5e0_71a2_4d8c_3f06);

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
}
