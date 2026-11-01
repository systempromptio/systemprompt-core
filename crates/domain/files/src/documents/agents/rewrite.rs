//! [`RewriteTracker`]: the nodes one agent has rewritten in one document
//! over the last hour, so a document replaced in several smaller commits
//! counts as replaced.
//!
//! Each `(document, agent)` keeps the node ids its rewrites touched, with
//! when; the distinct set inside the hour is what the broad-rewrite rule
//! weighs against the document's size. Held in process memory and bounded
//! like the edit ceiling: at most [`MAX_TRACKED`] keys, emptied windows
//! evicted first, then the least recently used.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use systemprompt_identifiers::{AgentName, DocumentId, DocumentNodeId};

use super::rate::MAX_TRACKED;

const WINDOW: Duration = Duration::from_secs(3600);

type Key = (DocumentId, String);

#[derive(Debug, Default)]
struct Touches {
    entries: VecDeque<(Instant, Vec<DocumentNodeId>)>,
    used: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct RewriteTracker {
    windows: Mutex<HashMap<Key, Touches>>,
}

impl RewriteTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn would_reach(
        &self,
        document: &DocumentId,
        agent: &AgentName,
        nodes: &HashSet<&DocumentNodeId>,
        now: Instant,
    ) -> usize {
        let windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = (document.clone(), agent.as_str().to_owned());
        let mut distinct: HashSet<&DocumentNodeId> = nodes.iter().copied().collect();
        if let Some(touches) = windows.get(&key) {
            distinct.extend(
                touches
                    .entries
                    .iter()
                    .filter(|(at, _)| now.saturating_duration_since(*at) < WINDOW)
                    .flat_map(|(_, nodes)| nodes.iter()),
            );
        }
        let reached = distinct.len();
        drop(distinct);
        drop(windows);
        reached
    }

    pub fn touch(
        &self,
        document: &DocumentId,
        agent: &AgentName,
        nodes: &HashSet<&DocumentNodeId>,
        now: Instant,
    ) -> usize {
        let mut windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = (document.clone(), agent.as_str().to_owned());
        if !windows.contains_key(&key) && windows.len() >= MAX_TRACKED {
            evict(&mut windows, now);
        }
        let touches = windows.entry(key).or_default();
        while touches
            .entries
            .front()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) >= WINDOW)
        {
            touches.entries.pop_front();
        }
        touches
            .entries
            .push_back((now, nodes.iter().map(|node| (*node).clone()).collect()));
        touches.used = Some(now);
        let distinct = touches
            .entries
            .iter()
            .flat_map(|(_, nodes)| nodes.iter())
            .collect::<HashSet<_>>()
            .len();
        drop(windows);
        distinct
    }
}

fn evict(windows: &mut HashMap<Key, Touches>, now: Instant) {
    windows.retain(|_, touches| {
        touches
            .entries
            .back()
            .is_some_and(|(at, _)| now.saturating_duration_since(*at) < WINDOW)
    });
    if windows.len() < MAX_TRACKED {
        return;
    }
    let oldest = windows
        .iter()
        .min_by_key(|(_, touches)| touches.used)
        .map(|(key, _)| key.clone());
    if let Some(key) = oldest {
        windows.remove(&key);
    }
}
