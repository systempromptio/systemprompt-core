//! Server-side cache of Gemini `thoughtSignature` values keyed by
//! conversation and `tool_use` id.
//!
//! Gemini attaches an opaque signature to each function call that must be
//! echoed back verbatim on the next turn. The gateway forwards it to
//! Anthropic-protocol clients as a non-standard `signature` field on
//! `tool_use` blocks, but strict clients drop unknown fields when replaying
//! history. This cache captures signatures as responses pass through and
//! re-injects them on inbound requests whose `tool_use` blocks arrive without
//! one, so any faithful Anthropic client works against Gemini upstreams.
//! Keys are scoped by [`GatewayConversationId`] because `tool_use` ids on
//! inbound requests are client-supplied: without the scope, a caller could
//! read another conversation's cached signatures by guessing ids.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use systemprompt_identifiers::GatewayConversationId;
use systemprompt_models::wire::canonical::{CanonicalContent, CanonicalRequest, CanonicalResponse};

const TTL: Duration = Duration::from_hours(1);
const MAX_ENTRIES: usize = 10_000;

struct Entry {
    signature: String,
    expires_at: Instant,
}

type Key = (GatewayConversationId, String);

pub struct ThoughtSignatureCache {
    entries: Mutex<HashMap<Key, Entry>>,
    ttl: Duration,
    max_entries: usize,
}

impl std::fmt::Debug for ThoughtSignatureCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThoughtSignatureCache")
            .field("ttl", &self.ttl)
            .field("max_entries", &self.max_entries)
            .finish_non_exhaustive()
    }
}

impl ThoughtSignatureCache {
    pub fn global() -> &'static Self {
        static CACHE: OnceLock<ThoughtSignatureCache> = OnceLock::new();
        CACHE.get_or_init(|| Self::new(TTL, MAX_ENTRIES))
    }

    #[must_use]
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            max_entries,
        }
    }

    pub fn store(&self, conversation: &GatewayConversationId, tool_use_id: &str, signature: &str) {
        let key = (conversation.clone(), tool_use_id.to_owned());
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = Instant::now();
        if !entries.contains_key(&key) && entries.len() >= self.max_entries {
            entries.retain(|_, e| e.expires_at > now);
            if entries.len() >= self.max_entries
                && let Some(oldest) = entries
                    .iter()
                    .min_by_key(|(_, e)| e.expires_at)
                    .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            key,
            Entry {
                signature: signature.to_owned(),
                expires_at: now + self.ttl,
            },
        );
    }

    pub fn lookup(
        &self,
        conversation: &GatewayConversationId,
        tool_use_id: &str,
    ) -> Option<String> {
        let key = (conversation.clone(), tool_use_id.to_owned());
        let now = Instant::now();
        let mut entries = self.entries.lock().ok()?;
        let entry = entries.get_mut(&key)?;
        if entry.expires_at <= now {
            entries.remove(&key);
            return None;
        }
        entry.expires_at = now + self.ttl;
        let signature = entry.signature.clone();
        drop(entries);
        Some(signature)
    }

    pub fn store_from_response(
        &self,
        conversation: &GatewayConversationId,
        response: &CanonicalResponse,
    ) {
        for content in &response.content {
            if let CanonicalContent::ToolUse {
                id,
                signature: Some(signature),
                ..
            } = content
            {
                self.store(conversation, id, signature);
            }
        }
    }

    pub fn hydrate_request(
        &self,
        conversation: &GatewayConversationId,
        request: &mut CanonicalRequest,
    ) {
        for message in &mut request.messages {
            for content in &mut message.content {
                let CanonicalContent::ToolUse { id, signature, .. } = content else {
                    continue;
                };
                match signature {
                    Some(sig) => self.store(conversation, id, sig),
                    None => {
                        if let Some(cached) = self.lookup(conversation, id) {
                            tracing::debug!(
                                tool_use_id = %id,
                                "re-injected cached thought signature into tool_use block"
                            );
                            *signature = Some(cached);
                        }
                    },
                }
            }
        }
    }
}
