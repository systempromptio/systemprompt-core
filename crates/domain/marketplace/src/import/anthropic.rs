//! Inbound Anthropic authoring formats the importer reads.
//!
//! [`MarketplaceJson`] is `.claude-plugin/marketplace.json` as Claude Code
//! defines it, and [`MarketplacePluginEntry`] one of its plugin records. Only
//! `name` is required on an entry; every other key is optional and several are
//! ignored by systemprompt because the equivalent fact is derived from the
//! tree. `source` is kept as an opaque value because Anthropic permits both a
//! relative path string and a git/object form;
//! [`MarketplacePluginEntry::plugin_source`] reads it into a
//! [`PluginSource`]: a string is a path inside the tree; an object whose
//! `source` is `github`, `url` or `git-subdir` is remote and fetched at import;
//! `npm` and `pip` cannot be vendored. Any other object keeps the older
//! reading of its `path` or `source` string as a local path.
//!
//! [`HooksFile`] is the `hooks/hooks.json` a plugin ships, whose body is the
//! same `HookEventsConfig` core already models.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::services::hooks::HookEventsConfig;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarketplaceOwner {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MarketplaceMetadata {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "pluginRoot", alias = "plugin_root")]
    pub plugin_root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceJson {
    pub name: String,
    #[serde(default)]
    pub owner: MarketplaceOwner,
    #[serde(default)]
    pub metadata: MarketplaceMetadata,
    #[serde(default)]
    pub plugins: Vec<MarketplacePluginEntry>,
    #[serde(
        default,
        rename = "allowCrossMarketplaceDependenciesOn",
        alias = "allow_cross_marketplace_dependencies_on"
    )]
    pub allow_cross_marketplace_dependencies_on: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplacePluginEntry {
    pub name: String,
    #[serde(default)]
    // JSON: Anthropic marketplace.json permits a string or an object here
    pub source: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    // JSON: Anthropic marketplace.json permits a string or an object here
    pub author: Option<serde_json::Value>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    // JSON: Claude Code component path override, a string or an array of them
    pub skills: Option<serde_json::Value>,
}

impl MarketplacePluginEntry {
    pub fn author_name(&self) -> Option<String> {
        match self.author.as_ref()? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(map) => {
                map.get("name").and_then(|v| v.as_str()).map(str::to_owned)
            },
            _ => None,
        }
    }

    pub fn author_email(&self) -> Option<String> {
        match self.author.as_ref()? {
            serde_json::Value::Object(map) => {
                map.get("email").and_then(|v| v.as_str()).map(str::to_owned)
            },
            _ => None,
        }
    }

    pub fn local_path(&self) -> Option<&str> {
        match self.plugin_source() {
            Ok(PluginSource::Local(path)) => Some(path),
            _ => None,
        }
    }

    pub fn source_is_remote(&self) -> bool {
        !matches!(
            self.plugin_source(),
            Ok(PluginSource::Default | PluginSource::Local(_))
        )
    }

    pub fn plugin_source(&self) -> Result<PluginSource<'_>, String> {
        let Some(value) = self.source.as_ref() else {
            return Ok(PluginSource::Default);
        };
        let map = match value {
            serde_json::Value::String(path) => return Ok(PluginSource::Local(path)),
            serde_json::Value::Object(map) => map,
            _ => return Ok(PluginSource::Unsupported("non-object".to_owned())),
        };
        let text = |key: &str| map.get(key).and_then(serde_json::Value::as_str);
        let kind = text("source");
        match kind {
            Some("github" | "url" | "git-subdir") => {
                RemotePluginSource::from_object(kind.unwrap_or_default(), &text)
                    .map(PluginSource::Remote)
            },
            Some(other @ ("npm" | "pip")) => Ok(PluginSource::Unsupported(other.to_owned())),
            _ => Ok(text("path").or(kind).map_or_else(
                || PluginSource::Unsupported("unknown".to_owned()),
                PluginSource::Local,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSource<'a> {
    Default,
    Local(&'a str),
    Remote(RemotePluginSource),
    Unsupported(String),
}

/// A plugin Claude Code would fetch from git.
///
/// Carries the repository as an `https` URL, the subtree the plugin lives in,
/// and the pin. `commit` is the entry's `sha`, the only reference a
/// reproducible bundle can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginSource {
    pub repository: String,
    pub subdirectory: Option<String>,
    pub reference: Option<String>,
    pub commit: Option<String>,
}

impl RemotePluginSource {
    fn from_object<'v>(kind: &str, text: &dyn Fn(&str) -> Option<&'v str>) -> Result<Self, String> {
        let (location, subdirectory) = match kind {
            "github" => (text("repo").ok_or("a github source needs `repo`")?, None),
            "url" => (text("url").ok_or("a url source needs `url`")?, None),
            _ => (
                text("url").ok_or("a git-subdir source needs `url`")?,
                Some(text("path").ok_or("a git-subdir source needs `path`")?),
            ),
        };
        let subdirectory = subdirectory
            .map(|path| path.trim_matches('/').trim_start_matches("./"))
            .filter(|path| !path.is_empty() && *path != ".")
            .map(|path| {
                systemprompt_models::managed::validate_path(path)
                    .map(|()| path.to_owned())
                    .map_err(|error| format!("`path` {path:?} is not a relative path: {error}"))
            })
            .transpose()?;
        let commit = text("sha").map(str::to_owned);
        if let Some(sha) = &commit
            && !is_commit(sha)
        {
            return Err(format!("`sha` {sha:?} is not a full lowercase commit id"));
        }
        Ok(Self {
            repository: repository_url(location)?,
            subdirectory,
            reference: text("ref").map(str::to_owned),
            commit,
        })
    }
}

fn repository_url(location: &str) -> Result<String, String> {
    if let Some(rest) = location.strip_prefix("https://") {
        let host = rest.split('/').next().unwrap_or_default();
        if host.is_empty() || host.contains('@') || rest.chars().any(char::is_whitespace) {
            return Err(format!("{location:?} is not a public https repository URL"));
        }
        return Ok(location.to_owned());
    }
    let mut parts = location.split('/');
    let segment_ok = |part: Option<&str>| {
        part.is_some_and(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
    };
    if segment_ok(parts.next()) && segment_ok(parts.next()) && parts.next().is_none() {
        return Ok(format!(
            "https://github.com/{}.git",
            location.trim_end_matches(".git")
        ));
    }
    Err(format!(
        "{location:?} is neither `owner/repository` nor an https URL"
    ))
}

fn is_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct HooksFile {
    #[serde(default)]
    pub(super) hooks: HookEventsConfig,
}
