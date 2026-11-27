//! Files projected from the manifest's plugin bundles into the config dir.
//!
//! A bundle's `agents/<name>.md` becomes `agents/<name>.md` with `mode:
//! subagent` first in its front matter (front matter is added when there is
//! none), and `hosts/<host>/themes/<name>.json` becomes `themes/<name>.json`
//! when a marketplace's host settings pin that theme at that file's
//! digest (a later marketplace's pin of a name wins). Only files one level deep
//! in those directories are taken. Plugins go in id order and the first one to
//! ship a path whose bytes have the manifest's SHA-256 takes it; a file not
//! delivered or not matching is skipped with a warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::names::{is_safe_plugin_segment, sha256_hex};
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::PluginEntry;

use crate::projection::input::BundleFiles;
use crate::projection::model::BundleFile;

const AGENTS_DIR: &str = "agents/";
const THEMES_DIR: &str = "themes/";
const FRONT_MATTER_END: &str = "\n---";

pub(crate) fn is_owned_path(path: &str) -> bool {
    if let Some(name) = path.strip_prefix(AGENTS_DIR) {
        return is_agent_name(name);
    }
    path.strip_prefix(THEMES_DIR)
        .and_then(|name| name.strip_suffix(".json"))
        .is_some_and(is_segment)
}

pub(crate) fn is_agent(path: &str) -> bool {
    path.starts_with(AGENTS_DIR)
}

pub(crate) fn wanted(manifest: &Manifest, host: HostKind) -> Vec<(PluginId, String, Sha256Digest)> {
    let pins = pins(manifest);
    let mut out = Vec::new();
    for plugin in plugins(manifest) {
        for file in &plugin.files {
            if target(host, &file.path, &file.sha256, &pins).is_some() {
                out.push((plugin.id.clone(), file.path.clone(), file.sha256.clone()));
            }
        }
    }
    out
}

pub(crate) fn project(
    manifest: &Manifest,
    host: HostKind,
    delivered: &BundleFiles,
    warnings: &mut Vec<String>,
) -> Vec<BundleFile> {
    let pins = pins(manifest);
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for plugin in plugins(manifest) {
        for file in &plugin.files {
            let Some(to) = target(host, &file.path, &file.sha256, &pins) else {
                continue;
            };
            if files.contains_key(&to) {
                continue;
            }
            let Some(bytes) = delivered.get(&plugin.id, &file.path) else {
                warnings.push(format!(
                    "{} of plugin {} not projected: not delivered",
                    file.path,
                    plugin.id.as_str()
                ));
                continue;
            };
            let actual = sha256_hex(bytes);
            if actual != file.sha256.as_str() {
                warnings.push(format!(
                    "{} of plugin {} not projected: its SHA-256 {actual} is not the manifest's {}",
                    file.path,
                    plugin.id.as_str(),
                    file.sha256.as_str()
                ));
                continue;
            }
            let bytes = if is_agent(&to) {
                as_subagent(&String::from_utf8_lossy(bytes)).into_bytes()
            } else {
                bytes.to_vec()
            };
            files.insert(to, bytes);
        }
    }
    files
        .into_iter()
        .map(|(path, bytes)| BundleFile { path, bytes })
        .collect()
}

fn as_subagent(markdown: &str) -> String {
    let body = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    if let Some(rest) = body.strip_prefix("---\n")
        && let Some(end) = rest.find(FRONT_MATTER_END)
        && let (Some(block), Some(tail)) =
            (rest.get(..end), rest.get(end + FRONT_MATTER_END.len()..))
    {
        let front: Vec<&str> = block
            .lines()
            .filter(|line| !line.trim_start().starts_with("mode:"))
            .collect();
        return format!("---\nmode: subagent\n{}\n---{tail}", front.join("\n"));
    }
    format!("---\nmode: subagent\n---\n\n{body}")
}

fn plugins(manifest: &Manifest) -> Vec<&PluginEntry> {
    let mut plugins: Vec<&PluginEntry> = manifest
        .plugins
        .iter()
        .filter(|plugin| is_safe_plugin_segment(plugin.id.as_str()))
        .collect();
    plugins.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    plugins
}

fn pins(manifest: &Manifest) -> BTreeMap<String, Sha256Digest> {
    let mut pins = BTreeMap::new();
    for settings in manifest.marketplace_settings() {
        for theme in &settings.themes {
            pins.insert(theme.name.clone(), theme.sha.clone());
        }
    }
    pins
}

fn target(
    host: HostKind,
    path: &str,
    sha: &Sha256Digest,
    pins: &BTreeMap<String, Sha256Digest>,
) -> Option<String> {
    if let Some(name) = path.strip_prefix(AGENTS_DIR) {
        return is_agent_name(name).then(|| path.to_owned());
    }
    let prefix = format!("hosts/{host}/{THEMES_DIR}");
    let name = path.strip_prefix(prefix.as_str())?.strip_suffix(".json")?;
    (is_segment(name) && pins.get(name) == Some(sha)).then(|| format!("{THEMES_DIR}{name}.json"))
}

fn is_agent_name(name: &str) -> bool {
    if !is_segment(name) || name.starts_with('.') {
        return false;
    }
    name.rsplit_once('.')
        .is_some_and(|(stem, extension)| !stem.is_empty() && extension.eq_ignore_ascii_case("md"))
}

fn is_segment(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}
