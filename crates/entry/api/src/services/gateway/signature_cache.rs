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
//! Signatures are persisted through [`AiThoughtSignatureRepository`] so a
//! replay served by a different replica, or by a process restarted since the
//! prior turn, still finds them; the in-memory map is a write-through L1 that
//! only spares the database round trip on the replica that captured the
//! signature. A miss on both tiers is a real failure mode: `thought_signature`
//! is omitted from the outbound wire when absent, and Gemini then rejects the
//! turn — so misses are counted under `gateway_signature_hydration_total` and
//! warned, but only when the resolved upstream is [`WireProtocol::Gemini`];
//! for every other wire the absent signature is expected and carries no
//! signal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_identifiers::GatewayConversationId;
use systemprompt_models::profile::WireProtocol;
use systemprompt_models::wire::canonical::{CanonicalContent, CanonicalRequest, CanonicalResponse};

pub const TTL: Duration = Duration::from_hours(1);
const HYDRATION_TOTAL: &str = "gateway_signature_hydration_total";
const CAPTURE_SKIPPED_TOTAL: &str = "gateway_signature_capture_skipped_total";

struct Entry {
    signature: String,
    expires_at: Instant,
}

type Key = (GatewayConversationId, String);

pub struct ThoughtSignatureCache {
    entries: Mutex<HashMap<Key, Entry>>,
    ttl: Duration,
    repository: Arc<AiThoughtSignatureRepository>,
}

impl std::fmt::Debug for ThoughtSignatureCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThoughtSignatureCache")
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl ThoughtSignatureCache {
    #[must_use]
    pub fn new(ttl: Duration, repository: Arc<AiThoughtSignatureRepository>) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            repository,
        }
    }

    fn store_local(&self, key: Key, signature: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.insert(
            key,
            Entry {
                signature: signature.to_owned(),
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    fn lookup_local(&self, key: &Key) -> Option<String> {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = entries.get_mut(key)?;
        if entry.expires_at <= now {
            entries.remove(key);
            return None;
        }
        entry.expires_at = now + self.ttl;
        let signature = entry.signature.clone();
        drop(entries);
        Some(signature)
    }

    pub async fn store(
        &self,
        conversation: &GatewayConversationId,
        tool_use_id: &str,
        signature: &str,
    ) {
        self.store_local((conversation.clone(), tool_use_id.to_owned()), signature);
        if let Err(e) = self
            .repository
            .upsert(conversation, tool_use_id, signature, self.ttl)
            .await
        {
            tracing::warn!(
                conversation = %conversation,
                tool_use_id = %tool_use_id,
                error = %e,
                "thought signature persisted only in this replica's memory"
            );
        }
    }

    pub async fn lookup(
        &self,
        conversation: &GatewayConversationId,
        tool_use_id: &str,
    ) -> Option<String> {
        let key = (conversation.clone(), tool_use_id.to_owned());
        if let Some(signature) = self.lookup_local(&key) {
            return Some(signature);
        }
        match self
            .repository
            .find(conversation, tool_use_id, self.ttl)
            .await
        {
            Ok(Some(signature)) => {
                self.store_local(key, &signature);
                Some(signature)
            },
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(
                    conversation = %conversation,
                    tool_use_id = %tool_use_id,
                    error = %e,
                    "thought signature lookup failed"
                );
                None
            },
        }
    }

    pub async fn store_from_response(
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
                self.store(conversation, id, signature).await;
            }
        }
    }

    pub async fn hydrate_request(
        &self,
        conversation: &GatewayConversationId,
        request: &mut CanonicalRequest,
        wire: Option<WireProtocol>,
    ) {
        let model = request.model.clone();
        let signatures_required = wire == Some(WireProtocol::Gemini);
        for message in &mut request.messages {
            for content in &mut message.content {
                let CanonicalContent::ToolUse { id, signature, .. } = content else {
                    continue;
                };
                match signature {
                    Some(sig) => self.store(conversation, id, sig).await,
                    None => match self.lookup(conversation, id).await {
                        Some(cached) => {
                            *signature = Some(cached);
                            if signatures_required {
                                metrics::counter!(HYDRATION_TOTAL, "outcome" => "hit").increment(1);
                            }
                        },
                        None => {
                            if signatures_required {
                                metrics::counter!(HYDRATION_TOTAL, "outcome" => "miss")
                                    .increment(1);
                                tracing::warn!(
                                    conversation = %conversation,
                                    tool_use_id = %id,
                                    model = %model,
                                    "no cached thought signature for tool_use; upstream may reject the turn"
                                );
                            }
                        },
                    },
                }
            }
        }
    }

    #[cfg(feature = "test-api")]
    #[expect(
        clippy::panic,
        reason = "test-only seam, compiled out unless `test-api` is enabled"
    )]
    pub fn poison_lock(&self) {
        let _guard = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        panic!("poisoning the signature cache lock");
    }

    #[must_use]
    pub fn signed_tool_use_count(response: &CanonicalResponse) -> usize {
        response
            .content
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    CanonicalContent::ToolUse {
                        signature: Some(_),
                        ..
                    }
                )
            })
            .count()
    }

    pub fn note_uncacheable_response(response: &CanonicalResponse, reason: &'static str) {
        let signed = Self::signed_tool_use_count(response);
        if signed == 0 {
            return;
        }
        metrics::counter!(CAPTURE_SKIPPED_TOTAL, "reason" => reason).increment(1);
        tracing::warn!(
            reason,
            signed_tool_use_blocks = signed,
            "thought signatures could not be cached; a later turn in this conversation will miss"
        );
    }
}
