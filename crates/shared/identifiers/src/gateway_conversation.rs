//! Deterministic gateway conversation cache key.
//!
//! Distinct from [`crate::ContextId`] (a user-owned agent context, UUID v4)
//! and [`crate::ProviderRequestId`] (an opaque upstream provider trace).
//! `GatewayConversationId` is **always** `ctx_<16 lowercase hex>` derived
//! from an FNV-1a hash of a conversation prefix, so the same opening turn
//! maps to the same id across processes, hosts, and Rust versions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

const PREFIX: &str = "ctx_";

fn validate(value: &str) -> Result<(), IdValidationError> {
    if value.len() != PREFIX.len() + 16 {
        return Err(IdValidationError::invalid(
            "GatewayConversationId",
            "must be 'ctx_' followed by 16 hex characters",
        ));
    }
    if !value.starts_with(PREFIX) {
        return Err(IdValidationError::invalid(
            "GatewayConversationId",
            "missing 'ctx_' prefix",
        ));
    }
    if !value[PREFIX.len()..]
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(IdValidationError::invalid(
            "GatewayConversationId",
            "suffix must be lowercase hex",
        ));
    }
    Ok(())
}

crate::define_id!(GatewayConversationId, validated, schema, validate);

impl GatewayConversationId {
    /// Mint a deterministic id from a 64-bit prefix hash.
    ///
    /// The hash itself is computed by `systemprompt_models::gateway_hash`
    /// helpers; see `conversation_prefix_hash`.
    #[must_use]
    pub fn from_prefix_hash(hash: u64) -> Self {
        Self::new(format!("{PREFIX}{hash:016x}"))
    }
}
