use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use systemprompt_identifiers::{GatewayConversationId, SessionId};
use systemprompt_models::gateway_hash::conversation_prefix_hash;

const CONTEXT_CACHE_CAP: usize = 1024;

#[derive(Debug)]
pub struct SessionContext {
    session_id: SessionId,
    contexts: Mutex<HashMap<u64, GatewayConversationId>>,
    last_activity_unix_ms: AtomicI64,
}

impl Default for SessionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: SessionId::generate(),
            contexts: Mutex::new(HashMap::with_capacity(64)),
            last_activity_unix_ms: AtomicI64::new(0),
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn touch_activity(&self) {
        let ms = Utc::now().timestamp_millis();
        self.last_activity_unix_ms.store(ms, Ordering::Relaxed);
    }

    #[must_use]
    pub fn last_activity(&self) -> Option<DateTime<Utc>> {
        let ms = self.last_activity_unix_ms.load(Ordering::Relaxed);
        if ms == 0 {
            None
        } else {
            DateTime::<Utc>::from_timestamp_millis(ms)
        }
    }

    pub fn context_for_prefix(&self, hash: u64) -> GatewayConversationId {
        let mut map = self.contexts.lock();
        if let Some(existing) = map.get(&hash) {
            return existing.clone();
        }
        if map.len() >= CONTEXT_CACHE_CAP {
            map.clear();
        }
        let ctx = GatewayConversationId::from_prefix_hash(hash);
        map.insert(hash, ctx.clone());
        ctx
    }
}

/// Stable conversation-prefix hash from a request body, recognising Anthropic
/// Messages, `OpenAI` Chat, and `OpenAI` Responses shapes; `None` when there is
/// no parseable first turn (the gateway then derives the id itself).
#[must_use]
pub fn derive_gateway_conversation_id(body: &[u8]) -> Option<u64> {
    let probe: PrefixProbe = serde_json::from_slice(body).ok()?;
    let (system, role, content) = probe.first_turn()?;
    Some(conversation_prefix_hash(system.as_deref(), &role, &content))
}

#[derive(serde::Deserialize)]
struct PrefixProbe {
    #[serde(default)]
    system: Option<serde_json::Value>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    messages: Option<Vec<ProbeMessage>>,
    #[serde(default)]
    input: Option<Vec<ProbeMessage>>,
}

#[derive(serde::Deserialize)]
struct ProbeMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

impl PrefixProbe {
    fn first_turn(&self) -> Option<(Option<String>, String, String)> {
        let messages = self.messages.as_deref().or(self.input.as_deref())?;
        let inline_system = messages
            .iter()
            .filter(|m| {
                m.role
                    .as_deref()
                    .is_some_and(|r| r.eq_ignore_ascii_case("system"))
            })
            .map(|m| content_text(m.content.as_ref()))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let inline_system = if inline_system.is_empty() {
            None
        } else {
            Some(inline_system.join("\n"))
        };
        let system = system_text(self.system.as_ref())
            .or_else(|| self.instructions.clone())
            .or(inline_system);
        let first = messages.iter().find(|m| {
            m.role
                .as_deref()
                .is_none_or(|r| !r.eq_ignore_ascii_case("system"))
        })?;
        let role = first.role.clone().unwrap_or_else(|| "user".to_owned());
        let content = content_text(first.content.as_ref());
        Some((system, role, content))
    }
}

fn system_text(value: Option<&serde_json::Value>) -> Option<String> {
    let v = value?;
    Some(match v {
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    })
}

fn content_text(value: Option<&serde_json::Value>) -> String {
    let Some(v) = value else {
        return String::new();
    };
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(map) => map
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => v.to_string(),
    }
}
