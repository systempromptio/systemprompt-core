//! Stable conversation-prefix hash used to mint deterministic
//! [`GatewayConversationId`](systemprompt_identifiers::GatewayConversationId)s
//! on both sides of the bridge boundary.
//!
//! The hash is FNV-1a 64-bit over a length-prefixed sequence of
//! `(label, bytes)` segments. It is **not** cryptographic — it is a
//! collision-resistant cache key that the bridge proxy and the gateway
//! `InboundAdapter`s can compute independently and arrive at the same
//! gateway conversation id for the same first turn of a conversation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hash an ordered list of labelled byte segments with FNV-1a 64.
///
/// Each segment is mixed in as `label || 0x00 || len_le_u32 || bytes || 0xFF`
/// so that distinct segment boundaries cannot collide.
#[must_use]
pub fn fnv1a_segments(parts: &[(&str, &[u8])]) -> u64 {
    let mut hash = FNV_OFFSET;
    for (label, bytes) in parts {
        for b in label.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= 0;
        hash = hash.wrapping_mul(FNV_PRIME);
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        for b in len.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        for b in *bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= 0xFF;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[must_use]
pub fn conversation_prefix_hash(
    system: Option<&str>,
    first_role: &str,
    first_content: &str,
) -> u64 {
    let mut parts: Vec<(&str, &[u8])> = Vec::with_capacity(3);
    if let Some(sys) = system.filter(|s| !s.is_empty()) {
        parts.push(("system", sys.as_bytes()));
    }
    parts.push(("role", first_role.as_bytes()));
    parts.push(("content", first_content.as_bytes()));
    fnv1a_segments(&parts)
}
