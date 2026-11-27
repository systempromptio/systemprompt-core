//! Markdown files with an optional YAML block between `---` lines at the top.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Number, Value};
use serde_yaml::Value as Yaml;

use crate::config::error::ConfigError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Document {
    // JSON: the YAML block converted to JSON so settings and front matter share one reader.
    pub(crate) data: Map<String, Value>,
    pub(crate) body: String,
}

pub(crate) fn parse(path: &Path, text: &str) -> Result<Document, ConfigError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok(Document {
            data: Map::new(),
            body: String::new(),
        });
    };
    if first.trim_end() != "---" {
        return Ok(Document {
            data: Map::new(),
            body: text.to_owned(),
        });
    }
    let mut offset = first.len();
    let mut block = None;
    for line in lines {
        if line.trim_end() == "---" {
            block = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let Some((yaml_end, body_start)) = block else {
        return Err(ConfigError::UnterminatedFrontMatter {
            path: path.to_path_buf(),
            line: 1,
        });
    };
    let data = parse_yaml(path, &text[first.len()..yaml_end])?;
    Ok(Document {
        data,
        body: text[body_start..].to_owned(),
    })
}

// JSON: the YAML block as a JSON object.
fn parse_yaml(path: &Path, yaml: &str) -> Result<Map<String, Value>, ConfigError> {
    let blank = yaml.lines().all(|line| {
        let line = line.trim();
        line.is_empty() || line.starts_with('#')
    });
    if blank {
        return Ok(Map::new());
    }
    match serde_yaml::from_str::<Yaml>(yaml) {
        Ok(value) => into_map(path, value),
        Err(error) => {
            let repaired = quote_colon_values(yaml);
            if repaired != yaml
                && let Ok(value) = serde_yaml::from_str::<Yaml>(&repaired)
            {
                return into_map(path, value);
            }
            let line = error.location().map_or(1, |location| location.line()) + 1;
            Err(ConfigError::FrontMatter {
                path: path.to_path_buf(),
                line,
                source: error,
            })
        },
    }
}

// JSON: the YAML block as a JSON object.
fn into_map(path: &Path, value: Yaml) -> Result<Map<String, Value>, ConfigError> {
    match to_json(value) {
        Value::Object(map) => Ok(map),
        Value::Null => Ok(Map::new()),
        _ => Err(ConfigError::FrontMatterShape {
            path: path.to_path_buf(),
        }),
    }
}

// Why: hand-written front matter often has `description: Use when: ...`, which
// YAML refuses. A top-level plain value containing a colon is quoted and the
// block parsed again; the original error stands if that fails too.
fn quote_colon_values(yaml: &str) -> String {
    let lines: Vec<String> = yaml
        .lines()
        .map(|line| {
            if line.starts_with(char::is_whitespace) || line.starts_with('#') {
                return line.to_owned();
            }
            let Some((key, value)) = line.split_once(':') else {
                return line.to_owned();
            };
            let simple_key = !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let value = value.trim();
            let structured = value.starts_with(['"', '\'', '|', '>', '[', '{', '&', '*', '!', '#']);
            if !simple_key || value.is_empty() || structured || !value.contains(':') {
                return line.to_owned();
            }
            match serde_json::to_string(value) {
                Ok(quoted) => format!("{key}: {quoted}"),
                Err(error) => {
                    tracing::debug!(%error, "front matter value left unquoted");
                    line.to_owned()
                },
            }
        })
        .collect();
    lines.join("\n")
}

fn key_text(key: Yaml) -> String {
    match key {
        Yaml::String(text) => text,
        other => match to_json(other) {
            Value::String(text) => text,
            value => value.to_string(),
        },
    }
}

// JSON: YAML values mapped onto their JSON counterparts.
fn to_json(value: Yaml) -> Value {
    match value {
        Yaml::Null => Value::Null,
        Yaml::Bool(flag) => Value::Bool(flag),
        Yaml::Number(number) => number
            .as_i64()
            .map(Value::from)
            .or_else(|| number.as_u64().map(Value::from))
            .or_else(|| {
                number
                    .as_f64()
                    .and_then(Number::from_f64)
                    .map(Value::Number)
            })
            .unwrap_or(Value::Null),
        Yaml::String(text) => Value::String(text),
        Yaml::Sequence(items) => Value::Array(items.into_iter().map(to_json).collect()),
        Yaml::Mapping(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key_text(key), to_json(value)))
                .collect(),
        ),
        Yaml::Tagged(tagged) => {
            let tagged = *tagged;
            to_json(tagged.value)
        },
    }
}
