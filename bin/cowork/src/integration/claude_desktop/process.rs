use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn list_claude_processes() -> Vec<String> {
    let output = match Command::new("/bin/ps").args(["-Ao", "comm"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<String> = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            (lower.contains("/claude.app/")
                || lower.ends_with("/claude")
                || lower.contains("claude helper"))
                && !lower.contains("claude code")
        })
        .map(|s| s.trim().to_string())
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
