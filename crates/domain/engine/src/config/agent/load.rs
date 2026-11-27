//! Assembling the agent table: built-ins, then agent files in source order,
//! then settings overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use super::{AgentDef, AgentMode, AgentOverride, Origin};
use crate::error::ConfigError;
use crate::frontmatter;
use crate::fsio::Probe;
use crate::paths::Paths;
use crate::permission::Ruleset;

#[derive(Debug, Clone)]
pub(crate) struct AgentSource {
    pub(crate) dir: PathBuf,
    pub(crate) mode: AgentMode,
}

pub(crate) struct AgentInputs<'a> {
    pub(crate) sources: &'a [AgentSource],
    pub(crate) overrides: &'a IndexMap<String, AgentOverride>,
    pub(crate) paths: &'a Paths,
}

pub(crate) fn load(
    probe: &mut Probe,
    inputs: &AgentInputs<'_>,
    builtins: Vec<AgentDef>,
    problems: &mut Vec<ConfigError>,
) -> IndexMap<String, AgentDef> {
    let mut table = Table {
        agents: builtins
            .into_iter()
            .map(|agent| (agent.name.clone(), agent))
            .collect(),
        paths: inputs.paths,
    };
    for source in inputs.sources {
        for file in probe.markdown_files(&source.dir) {
            match read_file(probe, &file, &inputs.paths.home) {
                Ok(Some((name, patch))) => {
                    table.merge(name, &patch, source.mode, Origin::File(file));
                },
                Ok(None) => {},
                Err(problem) => problems.push(problem),
            }
        }
    }
    for (name, patch) in inputs.overrides {
        table.merge(name.clone(), patch, AgentMode::All, Origin::Settings);
    }
    table.agents
}

fn read_file(
    probe: &mut Probe,
    file: &Path,
    home: &Path,
) -> Result<Option<(String, AgentOverride)>, ConfigError> {
    let Some(text) = probe.read(file)? else {
        return Ok(None);
    };
    let document = frontmatter::parse(file, &text)?;
    let mut patch = AgentOverride::from_map(&document.data, home)
        .map_err(|(key, source)| ConfigError::field(file, key, source))?;
    let body = document.body.trim();
    if !body.is_empty() {
        patch.prompt = Some(body.to_owned());
    }
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = patch
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(stem);
    if name.is_empty() {
        return Err(ConfigError::missing(file, "name"));
    }
    Ok(Some((name, patch)))
}

struct Table<'a> {
    agents: IndexMap<String, AgentDef>,
    paths: &'a Paths,
}

impl Table<'_> {
    fn merge(&mut self, name: String, patch: &AgentOverride, mode: AgentMode, origin: Origin) {
        if patch.disable {
            self.agents.shift_remove(&name);
            return;
        }
        let paths = self.paths;
        let agent = self.agents.entry(name.clone()).or_insert_with(|| {
            let mut agent = AgentDef::new(name, mode);
            agent.defaults = Ruleset::build_defaults(paths);
            agent
        });
        agent.apply(patch);
        agent.origin = origin;
    }
}
