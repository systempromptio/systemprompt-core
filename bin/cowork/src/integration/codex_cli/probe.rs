use std::collections::BTreeMap;
use std::process::Command;

use super::config::{self, KEYS_OF_INTEREST};

#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
}

pub(super) fn read_config() -> DomainRead {
    let path = config::config_path();
    let mut out = DomainRead::default();

    let bytes = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return out,
    };
    out.source_path = Some(path.display().to_string());

    let value: toml::Value = match toml::from_str(&bytes) {
        Ok(v) => v,
        Err(_) => return out,
    };

    for dotted in KEYS_OF_INTEREST {
        if let Some(raw) = lookup_dotted(&value, dotted) {
            out.keys
                .insert((*dotted).to_string(), config::redact_if_sensitive(dotted, raw));
        }
    }

    out
}

fn lookup_dotted(root: &toml::Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        let key = segment.trim_matches('"');
        cur = cur.as_table()?.get(key)?;
    }
    Some(stringify(cur))
}

fn stringify(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(_) | toml::Value::Table(_) | toml::Value::Datetime(_) => v.to_string(),
    }
}

pub(super) fn list_codex_processes() -> Vec<String> {
    if cfg!(target_os = "windows") {
        list_windows()
    } else {
        list_unix()
    }
}

fn list_unix() -> Vec<String> {
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
            let trimmed = lower.trim_end();
            trimmed.ends_with("/codex") || trimmed == "codex"
        })
        .map(|s| s.trim().to_string())
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

fn list_windows() -> Vec<String> {
    let output = match Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let first = line.split(',').next()?.trim_matches('"').to_ascii_lowercase();
            if first == "codex.exe" {
                Some(first)
            } else {
                None
            }
        })
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

#[allow(dead_code)]
pub(super) fn detect_binary() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let exe_name = if cfg!(target_os = "windows") {
        "codex.exe"
    } else {
        "codex"
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

pub(super) fn write_dotted(target: &mut toml::Value, dotted: &str, value: toml::Value) -> bool {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = target;
    for segment in &segments[..segments.len() - 1] {
        let key = segment.trim_matches('"');
        let table = match cur {
            toml::Value::Table(t) => t,
            _ => return false,
        };
        if !table.contains_key(key) {
            table.insert(
                key.to_string(),
                toml::Value::Table(toml::map::Map::new()),
            );
        }
        cur = table
            .get_mut(key)
            .expect("inserted above");
        if !matches!(cur, toml::Value::Table(_)) {
            *cur = toml::Value::Table(toml::map::Map::new());
        }
    }
    let last = segments[segments.len() - 1].trim_matches('"');
    if let toml::Value::Table(t) = cur {
        t.insert(last.to_string(), value);
        true
    } else {
        false
    }
}

pub(super) fn read_dotted(root: &toml::Value, dotted: &str) -> Option<toml::Value> {
    let mut cur = root;
    for segment in dotted.split('.') {
        let key = segment.trim_matches('"');
        cur = cur.as_table()?.get(key)?;
    }
    Some(cur.clone())
}
