//! Marketplace hook-consent payloads for the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use super::{MarketplaceExtra, MarketplaceItem};

pub fn list_hooks(dir: &Path) -> Vec<MarketplaceItem> {
    let path = dir.join("hooks.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    hook_items(&bytes, &path)
}

pub fn hook_items(bytes: &[u8], path: &Path) -> Vec<MarketplaceItem> {
    use crate::sync::apply::hooks_schema::{HookEntry as WireHookEntry, HooksFile};

    let Ok(file) = serde_json::from_slice::<HooksFile>(bytes) else {
        return Vec::new();
    };

    let mut governed_events: BTreeSet<String> = BTreeSet::new();
    let mut tracked_events: BTreeSet<String> = BTreeSet::new();
    let mut user_rows: Vec<MarketplaceItem> = Vec::new();

    for (event, matchers) in &file.hooks {
        for matcher in matchers {
            for (i, entry) in matcher.hooks.iter().enumerate() {
                match entry {
                    WireHookEntry::Http { url, .. } => {
                        if url.contains("/hooks/govern") {
                            governed_events.insert(event.clone());
                        } else {
                            tracked_events.insert(event.clone());
                        }
                    },
                    WireHookEntry::Command {
                        command, r#async, ..
                    } => {
                        let name = if matcher.matcher == "*" {
                            event.clone()
                        } else {
                            format!("{event} ({})", matcher.matcher)
                        };
                        let summary = if r#async.unwrap_or(false) {
                            format!("{command} (async)")
                        } else {
                            command.clone()
                        };
                        user_rows.push(MarketplaceItem {
                            id: format!("{event}:{}:{i}", matcher.matcher),
                            name,
                            source: "tenant",
                            path: String::new(),
                            summary: Some(summary),
                            readme: None,
                            change: None,
                            extra: MarketplaceExtra::None,
                        });
                    },
                }
            }
        }
    }

    let mut out = Vec::new();

    if !governed_events.is_empty() || !tracked_events.is_empty() {
        let mut summary_parts = Vec::new();
        if !governed_events.is_empty() {
            let events: Vec<String> = governed_events.iter().cloned().collect();
            summary_parts.push(format!("Governing {}", events.join(", ")));
        }
        if !tracked_events.is_empty() {
            summary_parts.push(format!("tracking {} events", tracked_events.len()));
        }

        let mut readme = format!(
            "System hooks installed by {}.\n",
            crate::brand::brand().app_name
        );
        if !governed_events.is_empty() {
            let events: Vec<String> = governed_events.iter().cloned().collect();
            readme.push_str(&format!(
                "\nGovernance (PreToolUse policy):\n- {}\n",
                events.join("\n- ")
            ));
        }
        if !tracked_events.is_empty() {
            let events: Vec<String> = tracked_events.iter().cloned().collect();
            readme.push_str(&format!("\nTracking:\n- {}\n", events.join("\n- ")));
        }

        out.push(MarketplaceItem {
            id: "systemprompt-governance".to_owned(),
            name: "Governance & tracking (active)".to_owned(),
            source: "tenant",
            path: path.display().to_string(),
            summary: Some(summary_parts.join("; ")),
            readme: Some(readme),
            change: None,
            extra: MarketplaceExtra::None,
        });
    }

    user_rows.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(user_rows);
    out
}
