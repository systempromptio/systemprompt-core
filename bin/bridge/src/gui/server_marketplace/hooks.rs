use std::collections::BTreeSet;
use std::path::Path;

use super::{MarketplaceExtra, MarketplaceItem};

// Hooks live in one `hooks/hooks.json` per synced plugin (written by
// `sync::apply::hooks`), keyed by event then matcher, not one file per hook.
pub(super) fn list_hooks(dir: &Path) -> Vec<MarketplaceItem> {
    let path = dir.join("hooks.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    hook_items(&bytes, &path)
}

// Split from `list_hooks` so the parsing is exercisable without filesystem I/O.
fn hook_items(bytes: &[u8], path: &Path) -> Vec<MarketplaceItem> {
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

        let mut readme = String::from("System hooks installed by the systemprompt bridge.\n");
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
            id: "systemprompt-governance".to_string(),
            name: "Governance & tracking (active)".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{hook_items, list_hooks};
    use std::path::Path;

    // One govern entry on PreToolUse plus a track entry on each of the 12
    // tracked events, mirroring `sync::apply::hooks::write_hooks_json`.
    const SYSTEM_HOOKS_JSON: &str = r#"{
        "hooks": {
            "PreToolUse": [{"matcher": "*", "hooks": [
                {"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/govern", "headers": {}, "allowedEnvVars": [], "timeout": 10}]}],
            "PostToolUse": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PostToolUse"}]}],
            "PostToolUseFailure": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PostToolUseFailure"}]}],
            "SessionStart": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SessionStart"}]}],
            "SessionEnd": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SessionEnd"}]}],
            "UserPromptSubmit": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "UserPromptSubmit"}]}],
            "Stop": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "Stop"}]}],
            "SubagentStart": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SubagentStart"}]}],
            "SubagentStop": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SubagentStop"}]}],
            "Notification": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "Notification"}]}],
            "TaskCompleted": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "TaskCompleted"}]}],
            "TeammateIdle": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "TeammateIdle"}]}],
            "PermissionRequest": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PermissionRequest"}]}]
        }
    }"#;

    #[test]
    fn missing_file_yields_empty() {
        assert!(list_hooks(Path::new("/var/empty/does-not-exist/hooks")).is_empty());
    }

    #[test]
    fn unparseable_json_yields_empty() {
        assert!(hook_items(b"not json", Path::new("hooks.json")).is_empty());
    }

    #[test]
    fn system_hooks_collapse_to_one_summary_row() {
        let items = hook_items(SYSTEM_HOOKS_JSON.as_bytes(), Path::new("hooks.json"));
        assert_eq!(items.len(), 1, "system govern/track collapse into one row");
        let row = &items[0];
        assert_eq!(row.id, "systemprompt-governance");
        let summary = row.summary.as_deref().unwrap_or_default();
        assert!(
            summary.contains("Governing PreToolUse"),
            "summary: {summary}"
        );
        assert!(summary.contains("tracking 12 events"), "summary: {summary}");
    }

    #[test]
    fn user_command_hooks_get_their_own_rows() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/govern", "headers": {}, "allowedEnvVars": [], "timeout": 10}]}],
                "PostToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "echo hi", "timeout": 10, "event": "PostToolUse"}]}]
            }
        }"#;
        let items = hook_items(json.as_bytes(), Path::new("hooks.json"));
        assert_eq!(items.len(), 2, "system summary + one user row");
        assert_eq!(items[0].id, "systemprompt-governance");
        let user = &items[1];
        assert_eq!(user.name, "PostToolUse (Bash)");
        assert_eq!(user.summary.as_deref(), Some("echo hi"));
    }
}
