//! Harness content projected from the signed manifest into
//! `~/.config/opencode`, in both provider tiers.
//!
//! - Rules whose `hosts` name an `OpenCode` packaging go, ordered by id, into
//!   one bridge-owned delimited block of `AGENTS.md`; text outside the block is
//!   the user's.
//! - A plugin bundle's `agents/<name>.md` becomes `agents/<name>.md` with
//!   `mode: subagent`; the first plugin (by id) to ship a name wins.
//! - A plugin bundle's `hosts/opencode/plugins/*.{js,ts}` becomes
//!   `plugins/<plugin>-<file>`.
//! - Each of a marketplace's `opencode.themes` is copied from the bundle that
//!   ships `hosts/opencode/themes/<name>.json` with the pinned digest, and its
//!   `hosts/opencode/desktop-themes/<name>.json` beside it when present.
//!
//! Every bundle file is checked against its manifest digest before it is
//! projected; a file that does not match is not written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use systemprompt_models::bridge::host::HostKind;
pub use systemprompt_models::bridge::opencode::agents_md::{agents_block, splice_block};

use super::owned;
use crate::gateway::manifest::{PluginEntry, PluginFile, RuleEntry, SignedManifest};
use crate::host_sync::{ApplyError, HostSyncReport, HostWarningKind};

const AGENTS_FILE: &str = "AGENTS.md";
const BUNDLE_PLUGINS: &str = "hosts/opencode/plugins/";
const BUNDLE_THEMES: &str = "hosts/opencode/themes/";
// Why: OpenCode Desktop has its own theme format and does not load external
// themes yet; the files are projected so a kit need not change once it does.
const BUNDLE_DESKTOP_THEMES: &str = "hosts/opencode/desktop-themes/";

pub(super) fn write_agents_md(root: &Path, rules: &[RuleEntry]) -> Result<(), ApplyError> {
    let path = root.join(AGENTS_FILE);
    let existing = crate::fsutil::read_optional(&path)
        .map_err(|source| ApplyError::Io {
            context: format!("read {}", path.display()),
            source,
        })?
        .unwrap_or_default();
    let next = splice_block(&existing, agents_block(rules).as_deref());
    if next == existing {
        return Ok(());
    }
    if next.is_empty() {
        return owned::remove(&path);
    }
    std::fs::create_dir_all(root).map_err(|source| ApplyError::Io {
        context: format!("create {}", root.display()),
        source,
    })?;
    crate::fsutil::atomic_write_0644(&path, next.as_bytes()).map_err(|source| ApplyError::Io {
        context: format!("write {}", path.display()),
        source,
    })
}

#[must_use]
pub fn as_subagent(markdown: &str) -> String {
    let body = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    if let Some(rest) = body.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        let front: Vec<&str> = rest[..end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("mode:"))
            .collect();
        let after = &rest[end + "\n---".len()..];
        return format!("---\nmode: subagent\n{}\n---{after}", front.join("\n"));
    }
    format!("---\nmode: subagent\n---\n\n{body}")
}

fn verified(
    plugin_root: &Path,
    plugin: &PluginEntry,
    file: &PluginFile,
) -> Result<Vec<u8>, ApplyError> {
    if !owned::is_plain_relative(&file.path) {
        return Err(ApplyError::UnsafePath(file.path.clone()));
    }
    let path = plugin_root.join(plugin.id.as_str()).join(&file.path);
    let bytes = std::fs::read(&path).map_err(|source| ApplyError::Io {
        context: format!("read {}", path.display()),
        source,
    })?;
    let actual = crate::hash::sha256_hex(&bytes);
    if actual != file.sha256.as_str() {
        return Err(ApplyError::HashMismatch {
            what: path.display().to_string(),
            expected: file.sha256.clone(),
            actual,
        });
    }
    Ok(bytes)
}

pub(super) fn bundle_files(
    manifest: &SignedManifest,
    plugin_root: &Path,
    report: &mut HostSyncReport,
) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut plugins: Vec<&PluginEntry> = manifest.plugins.iter().collect();
    plugins.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let themes: BTreeMap<&str, &str> = manifest
        .marketplaces
        .iter()
        .filter_map(|m| m.opencode.as_ref())
        .flat_map(|o| &o.themes)
        .map(|t| (t.name.as_str(), t.sha.as_str()))
        .collect();
    for plugin in plugins {
        for file in &plugin.files {
            let Some(target) = target_for(plugin, file, &themes) else {
                continue;
            };
            if out.contains_key(&target) {
                continue;
            }
            match verified(plugin_root, plugin, file) {
                Ok(bytes) => {
                    let bytes = if target.starts_with("agents/") {
                        as_subagent(&String::from_utf8_lossy(&bytes)).into_bytes()
                    } else {
                        bytes
                    };
                    out.insert(target, bytes);
                },
                Err(reason) => report.warn(
                    HostWarningKind::Manifest,
                    HostKind::OpenCode,
                    format!(
                        "{} of plugin {} not projected: {reason}",
                        file.path, plugin.id
                    ),
                ),
            }
        }
    }
    out
}

fn has_extension(name: &str, extensions: &[&str]) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| extensions.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

fn target_for(
    plugin: &PluginEntry,
    file: &PluginFile,
    themes: &BTreeMap<&str, &str>,
) -> Option<String> {
    let path = file.path.as_str();
    if let Some(name) = path.strip_prefix("agents/")
        && !name.contains('/')
        && has_extension(name, &["md"])
    {
        return Some(format!("agents/{name}"));
    }
    if let Some(name) = path.strip_prefix(BUNDLE_PLUGINS)
        && !name.contains('/')
        && has_extension(name, &["js", "ts"])
    {
        return Some(format!("plugins/{}-{name}", plugin.id.as_str()));
    }
    if let Some(name) = path
        .strip_prefix(BUNDLE_DESKTOP_THEMES)
        .and_then(|rest| rest.strip_suffix(".json"))
    {
        return themes
            .contains_key(name)
            .then(|| format!("desktop-themes/{name}.json"));
    }
    let name = path.strip_prefix(BUNDLE_THEMES)?.strip_suffix(".json")?;
    (themes.get(name) == Some(&file.sha256.as_str())).then(|| format!("themes/{name}.json"))
}
