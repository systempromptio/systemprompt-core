//! Slash commands: markdown templates whose body receives the user's arguments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::{ConfigError, FieldError};
use crate::frontmatter;
use crate::fsio::Probe;

/// A slash command loaded from `<dir>/<name>.md`; nested directories give names
/// like `git/commit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub subtask: Option<bool>,
    pub template: String,
    pub location: PathBuf,
}

impl Command {
    pub fn render(&self, arguments: &str) -> String {
        expand_template(&self.template, arguments)
    }
}

pub fn split_arguments(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    for c in input.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            },
            None if c.is_whitespace() => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            },
            None => {
                current.push(c);
                started = true;
            },
        }
    }
    if started {
        out.push(current);
    }
    out
}

pub fn expand_template(template: &str, arguments: &str) -> String {
    let words = split_arguments(arguments);
    let raw = arguments.trim();
    let mut out = String::with_capacity(template.len() + raw.len());
    let mut substituted = false;
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if let Some(tail) = after.strip_prefix("ARGUMENTS") {
            out.push_str(raw);
            substituted = true;
            rest = tail;
            continue;
        }
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        let index = after[..digits].parse::<usize>().unwrap_or(0);
        if index >= 1 {
            out.push_str(words.get(index - 1).map_or("", String::as_str));
            substituted = true;
            rest = &after[digits..];
            continue;
        }
        out.push('$');
        rest = after;
    }
    out.push_str(rest);
    if !substituted && !raw.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(raw);
    }
    out
}

pub(crate) fn load(
    probe: &mut Probe,
    dirs: &[PathBuf],
    problems: &mut Vec<ConfigError>,
) -> IndexMap<String, Command> {
    let mut commands = IndexMap::new();
    for dir in dirs {
        for file in probe.markdown_files(dir) {
            match read_command(probe, dir, &file) {
                Ok(Some(command)) => {
                    commands.insert(command.name.clone(), command);
                },
                Ok(None) => {},
                Err(problem) => problems.push(problem),
            }
        }
    }
    commands.sort_keys();
    commands
}

fn command_name(dir: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(dir).ok()?.with_extension("");
    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    let name = parts.join("/");
    (!name.is_empty()).then_some(name)
}

// JSON: an optional front matter value read as text.
fn optional_text(
    path: &Path,
    key: &str,
    value: Option<&Value>,
) -> Result<Option<String>, ConfigError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ConfigError::field(
            path,
            key,
            FieldError::Type {
                expected: "a string",
            },
        )),
    }
}

fn read_command(
    probe: &mut Probe,
    dir: &Path,
    file: &Path,
) -> Result<Option<Command>, ConfigError> {
    let Some(name) = command_name(dir, file) else {
        return Ok(None);
    };
    let Some(text) = probe.read(file)? else {
        return Ok(None);
    };
    let document = frontmatter::parse(file, &text)?;
    let data = &document.data;
    let subtask = match data.get("subtask") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(flag)) => Some(*flag),
        Some(_) => {
            let refused = FieldError::Type {
                expected: "true or false",
            };
            return Err(ConfigError::field(file, "subtask", refused));
        },
    };
    Ok(Some(Command {
        name,
        description: optional_text(file, "description", data.get("description"))?,
        agent: optional_text(file, "agent", data.get("agent"))?,
        model: optional_text(file, "model", data.get("model"))?,
        subtask,
        template: document.body.trim().to_owned(),
        location: file.to_path_buf(),
    }))
}
