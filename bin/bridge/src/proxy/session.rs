use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use systemprompt_identifiers::{ContextId, SessionId};

const CONTEXT_CACHE_CAP: usize = 1024;

#[derive(Debug)]
pub struct SessionContext {
    session_id: SessionId,
    contexts: Mutex<HashMap<u64, ContextId>>,
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
    pub fn session_id(&self) -> &SessionId {
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

    pub fn context_for_prefix(&self, hash: u64) -> ContextId {
        let mut map = self.contexts.lock();
        if let Some(existing) = map.get(&hash) {
            return existing.clone();
        }
        if map.len() >= CONTEXT_CACHE_CAP {
            map.clear();
        }
        let ctx = ContextId::generate();
        map.insert(hash, ctx.clone());
        ctx
    }
}

#[must_use]
pub fn hash_conversation_prefix(body: &[u8]) -> Option<u64> {
    let probe: PrefixProbe = serde_json::from_slice(body).ok()?;
    let first_message = probe.messages.first()?;
    let mut hasher = DefaultHasher::new();
    if let Some(system) = probe.system.as_ref() {
        b"system".hash(&mut hasher);
        system.get().as_bytes().hash(&mut hasher);
    }
    b"messages[0]".hash(&mut hasher);
    first_message.get().as_bytes().hash(&mut hasher);
    Some(hasher.finish())
}

#[derive(serde::Deserialize)]
struct PrefixProbe<'a> {
    #[serde(default, borrow)]
    system: Option<&'a serde_json::value::RawValue>,
    #[serde(default, borrow)]
    messages: Vec<&'a serde_json::value::RawValue>,
}
