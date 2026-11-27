//! What a projection produces: the files the applier writes, and the snapshot
//! other crates read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};
use sp_protocol::ids::{InstanceKey, McpServerId, PluginId, SkillId};
use sp_protocol::view::SyncCounts;
use sp_protocol::wire::manifest::{Publication, ThemeMode, ToolPolicy};

/// One managed skill: its directory under `skills/` and the exact bytes of its
/// `SKILL.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    pub id: SkillId,
    pub dir: String,
    pub markdown: String,
}

/// The skills of a manifest that has any, ordered by directory; `marketplaces`
/// is recorded in the skills ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSet {
    pub files: Vec<SkillFile>,
    pub marketplaces: Vec<String>,
}

/// A file projected from a plugin bundle, relative to the config dir:
/// `agents/<name>.md` or `themes/<name>.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// A remote MCP server the instance manages.
///
/// `name` is the server's slug (tool names are
/// `<name>_<tool>`), and a user server of the same name is replaced by it.
/// `bearer` is true only for a server on the instance's own origin: the caller
/// adds the user's JWT as `Authorization` to that one only. `headers` are the
/// declared ones, without any `Authorization`; they may hold credentials, so
/// `Debug` shows their names only.
#[derive(Clone, PartialEq)]
pub struct ManagedMcp {
    pub id: McpServerId,
    pub name: String,
    pub url: String,
    pub bearer: bool,
    pub headers: BTreeMap<String, String>,
    pub tool_policy: BTreeMap<String, ToolPolicy>,
}

impl fmt::Debug for ManagedMcp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedMcp")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("url", &self.url)
            .field("bearer", &self.bearer)
            .field("headers", &self.headers.keys().collect::<Vec<_>>())
            .field("tool_policy", &self.tool_policy)
            .finish()
    }
}

/// The gateway route a model is called on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Messages,
    ChatCompletions,
}

impl Protocol {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Messages => "/messages",
            Self::ChatCompletions => "/chat/completions",
        }
    }
}

/// One model the gateway serves to this host. `families` are the client
/// families its provider is served to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayModel {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub context: Option<u64>,
    pub output: Option<u64>,
    pub families: Vec<String>,
}

/// The model list of the instance's gateway.
///
/// `base_url` is the account's URL plus `/v1` (never
/// the profile's internal address); `protocols` are the client families this
/// host may list, sent with each call as `x-inference-protocol`;
/// `default_model` is the profile's default without a bracketed suffix, when
/// listed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gateway {
    pub base_url: Option<String>,
    pub models: Vec<GatewayModel>,
    pub default_model: Option<String>,
    pub protocols: Vec<String>,
}

/// The theme a marketplace makes the default, offered only when its file was
/// projected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultTheme {
    pub name: String,
    pub mode: Option<ThemeMode>,
}

/// How much of the instance a projection holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub skills: u32,
    pub agents: u32,
    pub mcp: u32,
    pub rules: u32,
}

impl Counts {
    #[must_use]
    pub const fn sync_counts(self, warnings: Vec<String>) -> SyncCounts {
        SyncCounts {
            skills: self.skills,
            agents: self.agents,
            mcp: self.mcp,
            rules: self.rules,
            warnings,
        }
    }
}

/// A projected skill whose runtime files come from a signed publication bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPublication {
    pub skill: SkillId,
    pub dir: String,
    pub publication: Publication,
}

/// Everything one manifest projects for one host.
///
/// `skills` is `None` when the manifest has no
/// skills (or the host is not enabled): applying it removes every managed skill
/// and the skills ledger. `defaults` are the marketplaces' client defaults, the
/// first marketplace winning a key, with the default `model` as the bare id the
/// engine lists.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    pub enabled: bool,
    pub instance_url: String,
    pub instance_key: InstanceKey,
    pub skills: Option<SkillSet>,
    pub bundle_files: Vec<BundleFile>,
    pub rules_block: Option<String>,
    pub mcp: Vec<ManagedMcp>,
    // JSON: marketplace defaults are settings-file values, written verbatim by key.
    pub defaults: Map<String, Value>,
    pub default_theme: Option<DefaultTheme>,
    pub gateway: Gateway,
    pub governance_plugin: Option<PluginId>,
    pub skill_owners: BTreeMap<SkillId, PluginId>,
    pub publications: Vec<SkillPublication>,
    pub counts: Counts,
    pub warnings: Vec<String>,
}

impl Projection {
    pub(crate) fn empty(instance_url: &str, instance_key: &InstanceKey) -> Self {
        Self {
            enabled: false,
            instance_url: instance_url.to_owned(),
            instance_key: instance_key.clone(),
            skills: None,
            bundle_files: Vec::new(),
            rules_block: None,
            mcp: Vec::new(),
            defaults: Map::new(),
            default_theme: None,
            gateway: Gateway::default(),
            governance_plugin: None,
            skill_owners: BTreeMap::new(),
            publications: Vec::new(),
            counts: Counts::default(),
            warnings: Vec::new(),
        }
    }

    #[must_use]
    pub fn skill_dir(&self, skill: &SkillId) -> Option<&str> {
        self.skills
            .as_ref()?
            .files
            .iter()
            .find(|file| &file.id == skill)
            .map(|file| file.dir.as_str())
    }

    #[must_use]
    pub fn applied(&self) -> Applied {
        Applied {
            instance_url: self.instance_url.clone(),
            instance_key: self.instance_key.clone(),
            gateway: self.gateway.clone(),
            mcp: self.mcp.clone(),
            governance_plugin: self.governance_plugin.clone(),
            skill_owners: self.skill_owners.clone(),
            publications: self.publications.clone(),
            default_theme: self.default_theme.clone(),
            counts: self.counts,
        }
    }
}

/// What the applied instance offers, as plain data.
///
/// The gateway models and default, the managed MCP servers, the governance
/// plugin, which plugin owns each skill, the skills with runtime publications,
/// and the counts.
///
/// It holds no credential; the JWT is added by whoever calls the
/// gateway or a server whose `bearer` is set.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub instance_url: String,
    pub instance_key: InstanceKey,
    pub gateway: Gateway,
    pub mcp: Vec<ManagedMcp>,
    pub governance_plugin: Option<PluginId>,
    pub skill_owners: BTreeMap<SkillId, PluginId>,
    pub publications: Vec<SkillPublication>,
    pub default_theme: Option<DefaultTheme>,
    pub counts: Counts,
}
