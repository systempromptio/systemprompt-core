//! Hermes configuration probing via dotted-key YAML lookup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use super::config::{self, KEYS_OF_INTEREST};
use crate::sysproc;

pub(super) use crate::integration::config_read::{DomainRead, ForeignShape};

pub(super) fn read_config() -> DomainRead {
    let path = config::config_yaml_path();
    let source = path.display().to_string();
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_into_keys(&text, &source),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DomainRead::default(),
        Err(e) => DomainRead::unreadable(&source, &e),
    }
}

fn parse_into_keys(text: &str, source: &str) -> DomainRead {
    let value: serde_yaml::Value = match serde_yaml::from_str(text) {
        Ok(value) => value,
        Err(e) => return DomainRead::unreadable(source, &e),
    };
    DomainRead::collect(
        source,
        KEYS_OF_INTEREST,
        |dotted| lookup_dotted(&value, dotted),
        |_, raw| raw,
    )
}

fn lookup_dotted(root: &serde_yaml::Value, dotted: &str) -> Option<String> {
    let mut cur = root;
    for segment in dotted.split('.') {
        let key = segment.trim_matches('"');
        cur = cur
            .as_mapping()?
            .get(serde_yaml::Value::String(key.to_owned()))?;
    }
    Some(stringify(cur))
}

fn stringify(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => "null".to_owned(),
        serde_yaml::Value::Sequence(_)
        | serde_yaml::Value::Mapping(_)
        | serde_yaml::Value::Tagged(_) => serde_yaml::to_string(v)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

pub(super) fn list_hermes_processes() -> Result<Vec<String>, sysproc::SysprocError> {
    sysproc::find_processes("hermes")
}

pub(super) fn write_dotted(
    target: &mut serde_yaml::Value,
    dotted: &str,
    value: serde_yaml::Value,
) -> Result<(), ForeignShape> {
    let segments: Vec<&str> = dotted.split('.').collect();
    let mut cur = target;
    let mut walked = String::new();
    for segment in &segments[..segments.len() - 1] {
        let name = segment.trim_matches('"');
        let serde_yaml::Value::Mapping(map) = cur else {
            return Err(foreign(&walked, cur));
        };
        if !walked.is_empty() {
            walked.push('.');
        }
        walked.push_str(name);
        cur = map
            .entry(serde_yaml::Value::String(name.to_owned()))
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    }
    let last = serde_yaml::Value::String(segments[segments.len() - 1].trim_matches('"').to_owned());
    let serde_yaml::Value::Mapping(m) = cur else {
        return Err(foreign(&walked, cur));
    };
    m.insert(last, value);
    Ok(())
}

fn foreign(key: &str, found: &serde_yaml::Value) -> ForeignShape {
    let found = match found {
        serde_yaml::Value::Null => "null",
        serde_yaml::Value::Bool(_) => "a boolean",
        serde_yaml::Value::Number(_) => "a number",
        serde_yaml::Value::String(_) => "a string",
        serde_yaml::Value::Sequence(_) => "a list",
        serde_yaml::Value::Mapping(_) => "a mapping",
        serde_yaml::Value::Tagged(_) => "a tagged value",
    };
    ForeignShape {
        path: "config.yaml".to_owned(),
        key: if key.is_empty() {
            "<root>".to_owned()
        } else {
            key.to_owned()
        },
        found,
        expected: "a mapping",
    }
}
