//! A scenario directory read into what the runner needs: its settings from
//! `scenario.toml`, its messages from `prompt.txt`, its script with the
//! placeholders of the POSIX shell family resolved.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use toml::{Table, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOn {
    Ask,
    Text,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub on: ActionOn,
    pub index: usize,
    pub act: String,
    pub message: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub permission: String,
    pub pattern: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub reply: String,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: String,
    pub dir: PathBuf,
    pub products: Vec<String>,
    pub protocols: Vec<String>,
    pub mode: String,
    pub rules: Vec<Rule>,
    pub asks: Vec<Ask>,
    pub actions: Vec<Action>,
    pub title_reply: String,
    pub exit: String,
    pub absent_outside: Vec<String>,
    pub harness: Table,
    pub commands: BTreeMap<String, String>,
    pub messages: Vec<String>,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn text(table: &Table, key: &str) -> Option<String> {
    table.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn tables<'a>(table: &'a Table, key: &str) -> impl Iterator<Item = &'a Table> {
    table
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
}

fn action(entry: &Table) -> anyhow::Result<Action> {
    let on = match text(entry, "on").as_deref() {
        Some("ask") => ActionOn::Ask,
        Some("text") => ActionOn::Text,
        Some("idle") => ActionOn::Idle,
        other => anyhow::bail!("unknown action trigger {other:?}"),
    };
    let index = entry.get("index").and_then(Value::as_integer).unwrap_or(1);
    Ok(Action {
        on,
        index: usize::try_from(index)?,
        act: text(entry, "do").unwrap_or_default(),
        message: entry
            .get("message")
            .and_then(Value::as_integer)
            .map(u32::try_from)
            .transpose()?,
    })
}

impl Scenario {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(dir.join("scenario.toml")).context("scenario.toml")?;
        let toml: Table = toml::from_str(&source)?;
        let permission = toml
            .get("permission")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        let rules = tables(&permission, "rule")
            .map(|rule| Rule {
                permission: text(rule, "permission").unwrap_or_default(),
                pattern: text(rule, "pattern").unwrap_or_default(),
                action: text(rule, "action").unwrap_or_default(),
            })
            .collect();
        let asks = tables(&toml, "ask")
            .map(|ask| Ask {
                reply: text(ask, "reply").unwrap_or_default(),
                feedback: text(ask, "feedback"),
            })
            .collect();
        let actions = tables(&toml, "action")
            .map(action)
            .collect::<anyhow::Result<_>>()?;
        let commands = toml
            .get("command")
            .and_then(Value::as_table)
            .into_iter()
            .flatten()
            .filter_map(|(name, dialects)| {
                let posix = dialects.as_table().and_then(|table| text(table, "posix"))?;
                Some((name.clone(), posix))
            })
            .collect();
        let end = toml
            .get("end")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        let prompt = std::fs::read_to_string(dir.join("prompt.txt")).context("prompt.txt")?;
        let messages = prompt
            .split("\n---\n")
            .map(|message| message.trim_end_matches('\n').to_owned())
            .collect();
        Ok(Self {
            id: text(&toml, "id").context("scenario.toml has no id")?,
            dir: dir.to_path_buf(),
            products: strings(toml.get("products")),
            protocols: strings(toml.get("protocols")),
            mode: text(&permission, "mode").unwrap_or_else(|| "ask".to_owned()),
            rules,
            asks,
            actions,
            title_reply: toml
                .get("title_call")
                .and_then(Value::as_table)
                .and_then(|table| text(table, "text"))
                .unwrap_or_default(),
            exit: text(&end, "exit").unwrap_or_else(|| "ok".to_owned()),
            absent_outside: strings(end.get("absent_outside")),
            harness: toml
                .get("harness")
                .and_then(Value::as_table)
                .cloned()
                .unwrap_or_default(),
            commands,
            messages,
        })
    }

    /// The corpus directory of this checkout.
    pub fn corpus() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenarios")
    }

    pub fn by_id(id: &str) -> anyhow::Result<Self> {
        Self::load(&Self::corpus().join(id))
    }

    /// `{{protocol}}` and every `{{command.<name>}}` with its POSIX entry.
    pub fn substitutions(&self, protocol: &str) -> Vec<(String, String)> {
        let mut pairs = vec![("{{protocol}}".to_owned(), protocol.to_owned())];
        pairs.extend(
            self.commands
                .iter()
                .map(|(name, command)| (format!("{{{{command.{name}}}}}"), command.clone())),
        );
        pairs
    }

    /// `turns.json` with the command placeholders resolved, as JSON text.
    pub fn turns(&self, protocol: &str) -> anyhow::Result<String> {
        let mut turns =
            std::fs::read_to_string(self.dir.join("turns.json")).context("turns.json")?;
        for (placeholder, with) in self.substitutions(protocol) {
            let escaped = serde_json::to_string(&with)?;
            turns = turns.replace(&placeholder, escaped.trim_matches('"'));
        }
        Ok(turns)
    }

    pub fn verdicts(&self) -> anyhow::Result<Option<String>> {
        let path = self.dir.join("verdicts.json");
        Ok(path
            .is_file()
            .then(|| std::fs::read_to_string(path))
            .transpose()?)
    }

    /// The answers file of the headless run, one entry per `[[ask]]`.
    pub fn answers(&self) -> serde_json::Value {
        let permissions: Vec<serde_json::Value> = self
            .asks
            .iter()
            .map(|ask| match ask.reply.as_str() {
                "reject" => serde_json::json!({ "type": "reject", "feedback": ask.feedback }),
                reply => serde_json::json!({ "type": reply }),
            })
            .collect();
        serde_json::json!({ "permissions": permissions, "questions": [] })
    }

    /// The rules as the engine's global settings file holds them.
    pub fn settings(&self) -> serde_json::Value {
        let mut permission = serde_json::Map::new();
        for rule in &self.rules {
            let entry = permission
                .entry(rule.permission.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(patterns) = entry.as_object_mut() {
                patterns.insert(
                    rule.pattern.clone(),
                    serde_json::Value::String(rule.action.clone()),
                );
            }
        }
        serde_json::json!({ "permission": permission })
    }
}
