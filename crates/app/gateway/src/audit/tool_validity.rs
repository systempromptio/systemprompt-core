//! Tool-call argument validity: compiled input-schema validators for the
//! catalog a request offered, shared across requests that offer the same one.
//!
//! The verdict is recorded on each captured tool call and never gates a
//! request. Validators are keyed by a SHA-256 over the offered tool names and
//! input schemas, so clients that resend one catalog every turn compile it
//! once. The cache is bounded and evicts the oldest catalog first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sha2::{Digest, Sha256};
use systemprompt_ai::services::tools::ToolArgsValidator;

use crate::protocol::canonical::CanonicalTool;

/// Bounded, catalog-keyed store of compiled [`ToolArgsValidator`]s, owned by
/// [`crate::GatewayRepositories`].
#[derive(Debug)]
pub struct ToolValidityCache {
    capacity: usize,
    entries: Mutex<CacheEntries>,
}

#[derive(Debug, Default)]
struct CacheEntries {
    validators: HashMap<String, Arc<ToolArgsValidator>>,
    order: VecDeque<String>,
}

impl ToolValidityCache {
    pub const DEFAULT_CAPACITY: usize = 256;

    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Mutex::new(CacheEntries::default()),
        }
    }

    pub fn len(&self) -> usize {
        self.entries().validators.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn validators_for(&self, tools: &[CanonicalTool]) -> Arc<ToolArgsValidator> {
        if tools.is_empty() {
            return Arc::new(ToolArgsValidator::default());
        }
        let key = catalog_key(tools);
        let cached = self.entries().validators.get(&key).cloned();
        if let Some(hit) = cached {
            return hit;
        }
        let compiled = Arc::new(ToolArgsValidator::compile(
            tools
                .iter()
                .map(|tool| (tool.name.as_str(), Some(&tool.input_schema))),
        ));
        self.entries()
            .insert(key, Arc::clone(&compiled), self.capacity);
        compiled
    }

    // Why: the map holds only immutable compiled validators, so a panic while
    // the lock was held cannot leave it inconsistent.
    fn entries(&self) -> MutexGuard<'_, CacheEntries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for ToolValidityCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

impl CacheEntries {
    fn insert(&mut self, key: String, validator: Arc<ToolArgsValidator>, capacity: usize) {
        if self.validators.contains_key(&key) {
            return;
        }
        while self.validators.len() >= capacity {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.validators.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.validators.insert(key, validator);
    }
}

pub fn catalog_key(tools: &[CanonicalTool]) -> String {
    let mut hasher = Sha256::new();
    for tool in tools {
        hasher.update(tool.name.as_bytes());
        hasher.update([0]);
        hasher.update(tool.input_schema.to_string().as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}
