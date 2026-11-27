//! The pure mapping from a verified manifest to a [`Projection`].
//!
//! Entries are kept only when they target the host and have a plain id. Skills
//! need a body; their directory is the id lowercased with every run of other
//! characters one `-`, and two skills that share a directory, or an id that
//! cannot name one, are dropped with a warning (the first stays). The
//! manifest's `agents` are not projected: agents come from the plugin bundles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sp_protocol::host::{enables_host, entry_targets_host};
use sp_protocol::ids::{PluginId, SkillId};
use sp_protocol::names::{is_plain_id, skill_dir_name};
use sp_protocol::wire::manifest::{Manifest, Sha256Digest, SkillEntry};

use crate::projection::input::ProjectionInput;
use crate::projection::model::{Counts, Projection, SkillFile, SkillPublication, SkillSet};
use crate::projection::{bundle, defaults, gateway, mcp, rules, skill_md};

#[must_use]
pub fn plan(input: &ProjectionInput<'_>) -> Projection {
    let mut projection = Projection::empty(input.instance_url, input.instance_key);
    let manifest = input.manifest;
    if !enables_host(&manifest.enabled_hosts, input.host) {
        tracing::info!(
            host = input.host,
            "the manifest does not enable this host; nothing is projected"
        );
        return projection;
    }
    let mut warnings = Vec::new();
    let selected = select_skills(manifest, input.host, &mut warnings);
    let skills = skill_set(manifest, &selected, &mut warnings);
    let bundle_files = bundle::project(manifest, input.host, input.bundle_files, &mut warnings);
    let selected_rules = rules::select(&manifest.rules, input.host, &mut warnings);
    let rules_block = rules::block(&selected_rules);
    rules::check_budget(rules_block.as_deref(), &mut warnings);
    let mcp = mcp::project(manifest, input.instance_url, &mut warnings);
    let gateway = gateway::project(input, &mut warnings);
    let files = skills
        .as_ref()
        .map(|set| set.files.as_slice())
        .unwrap_or_default();
    let (publications, skill_owners) = skill_links(files, &selected);
    let counts = Counts {
        skills: count(files.len()),
        agents: count(
            bundle_files
                .iter()
                .filter(|file| bundle::is_agent(&file.path))
                .count(),
        ),
        mcp: count(mcp.len()),
        rules: count(selected_rules.len()),
    };
    for warning in &warnings {
        tracing::warn!(warning = %warning, "projection entry skipped");
    }
    projection.enabled = true;
    projection.defaults = defaults::project(manifest);
    projection.default_theme = defaults::theme(manifest, &bundle_files);
    projection.governance_plugin = manifest.governance_plugin().map(|plugin| plugin.id.clone());
    projection.skill_owners = skill_owners;
    projection.publications = publications;
    projection.skills = skills;
    projection.bundle_files = bundle_files;
    projection.rules_block = rules_block;
    projection.mcp = mcp;
    projection.gateway = gateway;
    projection.counts = counts;
    projection.warnings = warnings;
    projection
}

#[must_use]
pub fn wanted_bundle_files(
    manifest: &Manifest,
    host: &str,
) -> Vec<(PluginId, String, Sha256Digest)> {
    if !enables_host(&manifest.enabled_hosts, host) {
        return Vec::new();
    }
    bundle::wanted(manifest, host)
}

pub(crate) fn unique_plain<'m, T>(
    entries: &'m [T],
    id_of: fn(&T) -> &str,
    kind: &str,
    warnings: &mut Vec<String>,
) -> Vec<&'m T> {
    let mut kept: Vec<&'m T> = Vec::with_capacity(entries.len());
    for entry in entries {
        let id = id_of(entry);
        if !is_plain_id(id) {
            warnings.push(format!("{kind} {id:?} skipped: its id is not a plain id"));
            continue;
        }
        let earlier = kept.iter().position(|kept_entry| id_of(kept_entry) == id);
        match earlier.and_then(|at| kept.get_mut(at)) {
            Some(slot) => *slot = entry,
            None => kept.push(entry),
        }
    }
    kept
}

fn select_skills<'m>(
    manifest: &'m Manifest,
    host: &str,
    warnings: &mut Vec<String>,
) -> Vec<&'m SkillEntry> {
    unique_plain(
        &manifest.skills,
        |skill| skill.id.as_str(),
        "skill",
        warnings,
    )
    .into_iter()
    .filter(|skill| entry_targets_host(&skill.hosts, host))
    .filter(|skill| {
        skill
            .instructions
            .as_deref()
            .is_some_and(|body| !body.trim().is_empty())
    })
    .collect()
}

fn skill_set(
    manifest: &Manifest,
    selected: &[&SkillEntry],
    warnings: &mut Vec<String>,
) -> Option<SkillSet> {
    if manifest.skills.is_empty() {
        return None;
    }
    let mut files: Vec<SkillFile> = Vec::with_capacity(selected.len());
    for skill in selected {
        let id = skill.id.as_str();
        let dir = skill_dir_name(id);
        if !names_a_directory(id) || dir.is_empty() {
            warnings.push(format!(
                "skill {id:?} skipped: its id cannot name a directory"
            ));
            continue;
        }
        if let Some(first) = files.iter().find(|file| file.dir == dir) {
            warnings.push(format!(
                "skill {id:?} skipped: it shares the directory {dir:?} with skill {:?}",
                first.id.as_str()
            ));
            continue;
        }
        match skill_md::render(skill, &dir) {
            Ok(markdown) => files.push(SkillFile {
                id: skill.id.clone(),
                dir,
                markdown,
            }),
            Err(error) => {
                tracing::debug!(skill = id, %error, "front matter could not be rendered");
                warnings.push(format!(
                    "skill {id:?} skipped: its front matter cannot be rendered as YAML"
                ));
            },
        }
    }
    files.sort_by(|a, b| a.dir.cmp(&b.dir));
    let marketplaces = manifest
        .marketplaces
        .iter()
        .map(|marketplace| marketplace.id.as_str().to_owned())
        .collect();
    Some(SkillSet {
        files,
        marketplaces,
    })
}

fn skill_links(
    files: &[SkillFile],
    selected: &[&SkillEntry],
) -> (Vec<SkillPublication>, BTreeMap<SkillId, PluginId>) {
    let entry_of = |file: &SkillFile| selected.iter().copied().find(|skill| skill.id == file.id);
    let publications = files
        .iter()
        .filter_map(|file| {
            let publication = entry_of(file)?.publication.clone()?;
            Some(SkillPublication {
                skill: file.id.clone(),
                dir: file.dir.clone(),
                publication,
            })
        })
        .collect();
    let owners = files
        .iter()
        .filter_map(|file| owner(entry_of(file)?).map(|plugin| (file.id.clone(), plugin)))
        .collect();
    (publications, owners)
}

fn names_a_directory(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn owner(skill: &SkillEntry) -> Option<PluginId> {
    skill
        .source
        .as_ref()
        .and_then(|source| source.plugin.as_deref())
        .filter(|plugin| !plugin.is_empty())
        .map(PluginId::new)
        .or_else(|| skill.plugins.first().cloned())
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
