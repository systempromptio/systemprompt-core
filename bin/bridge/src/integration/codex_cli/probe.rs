use std::collections::BTreeMap;
use std::process::Command;

use super::config::{self, KEYS_OF_INTEREST};

#[derive(Debug, Clone, Default)]
pub(super) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
}

pub(super) fn read_config() -> DomainRead {
    let managed = config::managed_config_path();
    if managed.exists() {
        if let Ok(text) = std::fs::read_to_string(&managed) {
            if let Some(read) = parse_into_keys(&text, &managed.display().to_string()) {
                return read;
            }
        }
    }
    let user = config::user_config_path();
    if user.exists() {
        if let Ok(text) = std::fs::read_to_string(&user) {
            if let Some(read) = parse_into_keys(&text, &user.display().to_string()) {
                return read;
            }
        }
    }
    DomainRead::default()
}

fn parse_into_keys(text: &str, source: &str) -> Option<DomainRead> {
    let value: toml::Value = toml::from_str(text).ok()?;
    let mut out = DomainRead {
        source_path: Some(source.to_string()),
        keys: BTreeMap::new(),
    };
    for dotted in KEYS_OF_INTEREST {
        if let Some(raw) = lookup_dotted(&value, dotted) {
            out.keys.insert(
                (*dotted).to_string(),
                config::redact_if_sensitive(dotted, raw),
            );
        }
    }
    Some(out)
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
            trimmed.ends_with("/codex") || trimmed == "codex" || trimmed.contains("/codex.app/")
        })
        .map(|s| s.trim().to_string())
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

fn list_windows() -> Vec<String> {
    #[cfg(target_os = "windows")]
    let output = match crate::winproc::tasklist_command()
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    #[cfg(not(target_os = "windows"))]
    let output = match Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
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
            let first = line
                .split(',')
                .next()?
                .trim_matches('"')
                .to_ascii_lowercase();
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

pub(super) fn write_dotted(target: &mut toml::Value, dotted: &str, value: toml::Value) -> bool {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = target;
    for segment in &segments[..segments.len() - 1] {
        let key = segment.trim_matches('"');
        let table = match cur {
            toml::Value::Table(t) => t,
            _ => return false,
        };
        let entry = table
            .entry(key.to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if !matches!(entry, toml::Value::Table(_)) {
            *entry = toml::Value::Table(toml::map::Map::new());
        }
        cur = entry;
    }
    let last = segments[segments.len() - 1].trim_matches('"');
    if let toml::Value::Table(t) = cur {
        t.insert(last.to_string(), value);
        true
    } else {
        false
    }
}
