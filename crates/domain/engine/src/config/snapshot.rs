//! One directory's complete configuration, and how to tell when it is out of
//! date.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::agent::{self, AgentDef, AgentInputs, AgentMode, AgentSource};
use crate::command::{self, Command};
use crate::error::ConfigError;
use crate::fsio::{Fingerprint, Probe};
use crate::instruction::{self, Instruction};
use crate::paths::{Layout, Paths};
use crate::permission::{Action, EXTERNAL_DIRECTORY, Rule, Ruleset, inside};
use crate::settings::{self, Settings};
use crate::skill::{self, Skill};
use crate::worktree::Worktree;

/// Settings, agents, skills, commands and instructions for one working
/// directory.
///
/// The problems found on the way are kept too. A settings file that cannot be
/// parsed fails the whole load; a bad agent, skill or command file is left out
/// and listed in `problems`. `cwd` must be absolute, and loading does blocking
/// file IO.
///
/// The effective rules for an agent are, lowest first: the agent's defaults,
/// access to skill directories, the global then project settings, the agent's
/// own rules, and the session's rules.
#[derive(Debug)]
pub struct Loaded {
    pub cwd: PathBuf,
    pub worktree: Worktree,
    pub paths: Paths,
    pub layout: Layout,
    pub settings: Settings,
    pub settings_files: Vec<PathBuf>,
    pub agents: IndexMap<String, AgentDef>,
    pub skills: IndexMap<String, Skill>,
    pub commands: IndexMap<String, Command>,
    pub instructions: Vec<Instruction>,
    pub problems: Vec<ConfigError>,
    inputs: Vec<PathBuf>,
    fingerprint: Fingerprint,
}

struct Sources {
    settings: Vec<PathBuf>,
    agents: Vec<AgentSource>,
    skills: Vec<PathBuf>,
    commands: Vec<PathBuf>,
}

fn push_unique(list: &mut Vec<PathBuf>, path: PathBuf) {
    if !list.contains(&path) {
        list.push(path);
    }
}

fn sources(paths: &Paths, layout: &Layout, chain: &[PathBuf]) -> Sources {
    let compat = layout.compat_dirs;
    let home_claude = paths.home.join(".claude");
    let mut settings = vec![layout.global_settings(paths)];
    let mut agents: Vec<AgentSource> = Vec::new();
    let mut skills = Vec::new();
    let mut commands = Vec::new();
    let mut add_agents = |dir: PathBuf, mode: AgentMode| {
        if !agents.iter().any(|source| source.dir == dir) {
            agents.push(AgentSource { dir, mode });
        }
    };

    if compat {
        add_agents(home_claude.join("agents"), AgentMode::Subagent);
        push_unique(&mut skills, paths.home.join(".agents").join("skills"));
        push_unique(&mut skills, home_claude.join("skills"));
        push_unique(&mut commands, home_claude.join("commands"));
    }
    add_agents(paths.config.join("agents"), AgentMode::All);
    push_unique(&mut skills, paths.config.join("skills"));
    push_unique(&mut commands, paths.config.join("commands"));

    for dir in chain {
        for file in layout.project_settings(dir) {
            push_unique(&mut settings, file);
        }
        let project = layout.project_root(dir);
        if compat {
            let claude = dir.join(".claude");
            add_agents(claude.join("agents"), AgentMode::Subagent);
            push_unique(&mut skills, dir.join(".agents").join("skills"));
            push_unique(&mut skills, claude.join("skills"));
            push_unique(&mut commands, claude.join("commands"));
        }
        add_agents(project.join("agents"), AgentMode::All);
        push_unique(&mut skills, project.join("skills"));
        push_unique(&mut commands, project.join("commands"));
    }
    Sources {
        settings,
        agents,
        skills,
        commands,
    }
}

impl Loaded {
    pub fn load(
        paths: &Paths,
        layout: &Layout,
        cwd: &Path,
        builtins: Vec<AgentDef>,
    ) -> Result<Self, ConfigError> {
        let worktree = Worktree::detect(cwd);
        let chain = worktree.chain(cwd);
        let sources = sources(paths, layout, &chain);
        let mut probe = Probe::default();
        let mut problems = Vec::new();

        let (settings, settings_files) =
            settings::load(&mut probe, &sources.settings, &paths.home, &mut problems)?;
        let agent_inputs = AgentInputs {
            sources: &sources.agents,
            overrides: &settings.agents,
            paths,
        };
        let agents = agent::load(&mut probe, &agent_inputs, builtins, &mut problems);
        let skills = skill::discover(&mut probe, &sources.skills, &mut problems);
        let commands = command::load(&mut probe, &sources.commands, &mut problems);
        let instruction_inputs = instruction::Inputs {
            paths,
            layout,
            root: &worktree.root,
            chain: &chain,
            extra: &settings.instructions,
        };
        let instructions = instruction::collect(&mut probe, &instruction_inputs, &mut problems);

        for problem in &problems {
            tracing::warn!(error = %problem, "configuration input skipped");
        }
        let inputs = probe.into_inputs();
        let fingerprint = Fingerprint::of(&inputs);
        tracing::debug!(
            cwd = %cwd.display(),
            root = %worktree.root.display(),
            settings = settings_files.len(),
            agents = agents.len(),
            skills = skills.len(),
            commands = commands.len(),
            instructions = instructions.len(),
            problems = problems.len(),
            "configuration loaded"
        );
        Ok(Self {
            cwd: cwd.to_path_buf(),
            worktree,
            paths: paths.clone(),
            layout: layout.clone(),
            settings,
            settings_files,
            agents,
            skills,
            commands,
            instructions,
            problems,
            inputs,
            fingerprint,
        })
    }

    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub fn current_fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.inputs)
    }

    pub fn is_stale(&self) -> bool {
        self.current_fingerprint() != self.fingerprint
    }

    pub fn inputs(&self) -> &[PathBuf] {
        &self.inputs
    }

    pub fn agent(&self, name: &str) -> Option<&AgentDef> {
        self.agents.get(name)
    }

    pub fn default_agent(&self) -> Option<&AgentDef> {
        let usable = |agent: &&AgentDef| agent.mode.is_primary() && !agent.hidden;
        self.settings
            .default_agent
            .as_deref()
            .and_then(|name| self.agents.get(name))
            .filter(usable)
            .or_else(|| self.agents.values().find(usable))
    }

    pub fn primary_agents(&self) -> impl Iterator<Item = &AgentDef> {
        self.agents
            .values()
            .filter(|agent| agent.mode.is_primary() && !agent.hidden)
    }

    pub fn subagents(&self) -> impl Iterator<Item = &AgentDef> {
        self.agents
            .values()
            .filter(|agent| agent.mode.is_subagent() && !agent.hidden)
    }

    pub fn skill_access(&self) -> Ruleset {
        self.skills
            .values()
            .map(|skill| Rule::new(EXTERNAL_DIRECTORY, inside(&skill.dir), Action::Allow))
            .collect()
    }

    pub fn rules_for(&self, agent: &AgentDef, session: &Ruleset) -> Ruleset {
        let skills = self.skill_access();
        let settings = &self.settings.permission;
        Ruleset::layered(&[
            &agent.defaults,
            &skills,
            settings,
            &agent.permission,
            session,
        ])
    }

    pub fn evaluate(
        &self,
        agent: &AgentDef,
        session: &Ruleset,
        permission: &str,
        pattern: &str,
    ) -> Action {
        self.rules_for(agent, session).evaluate(permission, pattern)
    }
}
