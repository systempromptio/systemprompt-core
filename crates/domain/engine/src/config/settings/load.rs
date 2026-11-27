//! Reading the settings files in order and folding them into one [`Settings`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::raw::{RawMcp, RawSettings};
use super::{McpServer, Settings};
use crate::config::agent::AgentOverride;
use crate::config::error::{ConfigError, FieldError};
use crate::config::fsio::Probe;
use crate::config::jsonc::{self, merge_value};
use crate::config::permission::Ruleset;

struct Layer {
    // JSON: the keys of one settings file left after rules and lists are lifted out.
    rest: Map<String, Value>,
    permission: Ruleset,
    agent_rules: Vec<(String, Ruleset)>,
    instructions: Vec<String>,
    mcp_names: Vec<String>,
}

fn parse_layer(path: &Path, text: &str, home: &Path) -> Result<Layer, ConfigError> {
    let mut rest = jsonc::parse_object(path, text)?;
    if !rest.is_empty() {
        jsonc_parser::parse_to_serde_value::<RawSettings>(text, &jsonc::options())
            .map_err(|error| ConfigError::jsonc(path, error))?;
    }
    let permission = match rest.shift_remove("permission") {
        Some(value) => Ruleset::from_value(&value, home)
            .map_err(|e| ConfigError::field(path, "permission", e.into()))?,
        None => Ruleset::new(),
    };
    let agent_rules = take_agent_rules(path, &mut rest, home)?;
    let instructions = match rest.shift_remove("instructions") {
        Some(value) => serde_json::from_value::<Vec<String>>(value).map_err(|error| {
            tracing::debug!(file = %path.display(), %error, "instructions refused");
            let refused = FieldError::Type {
                expected: "a list of paths or globs",
            };
            ConfigError::field(path, "instructions", refused)
        })?,
        None => Vec::new(),
    };
    let mcp_names = rest
        .get("mcp")
        .and_then(Value::as_object)
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();
    Ok(Layer {
        rest,
        permission,
        agent_rules,
        instructions,
        mcp_names,
    })
}

// JSON: the parsed settings object, whose `agent.<name>.permission` entries are
// lifted out.
fn take_agent_rules(
    path: &Path,
    rest: &mut Map<String, Value>,
    home: &Path,
) -> Result<Vec<(String, Ruleset)>, ConfigError> {
    let mut agent_rules = Vec::new();
    let Some(Value::Object(agents)) = rest.get_mut("agent") else {
        return Ok(agent_rules);
    };
    for (name, entry) in agents.iter_mut() {
        let Value::Object(entry) = entry else {
            let refused = FieldError::Type {
                expected: "an object",
            };
            return Err(ConfigError::field(path, format!("agent.{name}"), refused));
        };
        if let Some(value) = entry.shift_remove("permission") {
            let rules = Ruleset::from_value(&value, home).map_err(|e| {
                ConfigError::field(path, format!("agent.{name}.permission"), e.into())
            })?;
            agent_rules.push((name.clone(), rules));
        }
        AgentOverride::from_map(entry, home).map_err(|(key, source)| {
            ConfigError::field(path, format!("agent.{name}.{key}"), source)
        })?;
    }
    Ok(agent_rules)
}

pub(crate) fn validate_text(path: &Path, text: &str, home: &Path) -> Result<(), ConfigError> {
    parse_layer(path, text, home).map(|_| ())
}

pub(crate) fn load(
    probe: &mut Probe,
    candidates: &[PathBuf],
    home: &Path,
    problems: &mut Vec<ConfigError>,
) -> Result<(Settings, Vec<PathBuf>), ConfigError> {
    let mut fold = Fold::new();
    for path in candidates {
        let Some(text) = probe.read(path)? else {
            continue;
        };
        let layer = parse_layer(path, &text, home)?;
        fold.add(path, layer);
        tracing::debug!(file = %path.display(), "settings file read");
    }
    fold.finish(home, problems)
}

struct Fold {
    // JSON: the deep merge of every file read so far, deserialized once at the end.
    merged: Value,
    files: Vec<PathBuf>,
    permission: Ruleset,
    agent_rules: IndexMap<String, Ruleset>,
    instructions: Vec<String>,
    origins: HashMap<String, PathBuf>,
}

impl Fold {
    fn new() -> Self {
        Self {
            merged: Value::Object(Map::new()),
            files: Vec::new(),
            permission: Ruleset::new(),
            agent_rules: IndexMap::new(),
            instructions: Vec::new(),
            origins: HashMap::new(),
        }
    }

    fn add(&mut self, path: &Path, layer: Layer) {
        self.permission.extend(&layer.permission);
        for (name, rules) in layer.agent_rules {
            self.agent_rules.entry(name).or_default().extend(&rules);
        }
        for item in layer.instructions {
            if !self.instructions.contains(&item) {
                self.instructions.push(item);
            }
        }
        for name in layer.mcp_names {
            self.origins.insert(name, path.to_path_buf());
        }
        merge_value(&mut self.merged, Value::Object(layer.rest));
        self.files.push(path.to_path_buf());
    }

    fn finish(
        self,
        home: &Path,
        problems: &mut Vec<ConfigError>,
    ) -> Result<(Settings, Vec<PathBuf>), ConfigError> {
        let raw: RawSettings =
            serde_json::from_value(self.merged).map_err(|source| ConfigError::Merged {
                files: self.files.clone(),
                source,
            })?;
        let last = self.files.last().cloned().unwrap_or_default();
        let agents = agent_overrides(raw.agent, self.agent_rules, home, &last)?;
        let mcp = mcp_servers(raw.mcp, &self.origins, &last, problems);
        let lsp = keep_runnable(
            raw.lsp,
            |server| (server.command.trim().is_empty(), server.disabled),
            "lsp",
            &last,
            problems,
        );
        let formatters = keep_runnable(
            raw.formatter,
            |format| (format.command.trim().is_empty(), format.disabled),
            "formatter",
            &last,
            problems,
        );
        let settings = Settings {
            model: raw.model,
            small_model: raw.small_model,
            default_agent: raw.default_agent,
            agents,
            permission: self.permission,
            mcp,
            lsp,
            formatters,
            shell: raw.shell.filter(|shell| !shell.trim().is_empty()),
            instructions: self.instructions,
            compaction: raw.compaction,
            experimental: raw.experimental,
            extra: raw.extra,
        };
        Ok((settings, self.files))
    }
}

// JSON: the merged `agent` section, one override object per agent name.
fn agent_overrides(
    raw: IndexMap<String, Map<String, Value>>,
    agent_rules: IndexMap<String, Ruleset>,
    home: &Path,
    last: &Path,
) -> Result<IndexMap<String, AgentOverride>, ConfigError> {
    let mut agents: IndexMap<String, AgentOverride> = IndexMap::new();
    for (name, map) in raw {
        let patch = AgentOverride::from_map(&map, home).map_err(|(key, source)| {
            ConfigError::field(last, format!("agent.{name}.{key}"), source)
        })?;
        agents.insert(name, patch);
    }
    for (name, rules) in agent_rules {
        agents.entry(name).or_default().permission.extend(&rules);
    }
    Ok(agents)
}

fn mcp_servers(
    raw: IndexMap<String, RawMcp>,
    origins: &HashMap<String, PathBuf>,
    last: &Path,
    problems: &mut Vec<ConfigError>,
) -> IndexMap<String, McpServer> {
    let mut mcp = IndexMap::new();
    for (name, server) in raw {
        match server.resolve() {
            Ok(server) => {
                mcp.insert(name, server);
            },
            Err(source) => {
                let path = origins.get(&name).map_or(last, PathBuf::as_path);
                problems.push(ConfigError::field(path, format!("mcp.{name}"), source));
            },
        }
    }
    mcp
}

fn keep_runnable<T>(
    entries: IndexMap<String, T>,
    state: impl Fn(&T) -> (bool, bool),
    section: &str,
    path: &Path,
    problems: &mut Vec<ConfigError>,
) -> IndexMap<String, T> {
    let mut kept = IndexMap::new();
    for (name, entry) in entries {
        match state(&entry) {
            (false, _) => {
                kept.insert(name, entry);
            },
            (true, true) => {},
            (true, false) => problems.push(ConfigError::field(
                path,
                format!("{section}.{name}.command"),
                FieldError::Missing { field: "command" },
            )),
        }
    }
    kept
}
