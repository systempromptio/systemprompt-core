//! Build-from-spec plugin bundle assembler.
//!
//! [`build_plugin_bundle`] turns a [`PluginConfig`] spec plus the instance's
//! already-resolved catalogue ([`BundleContent`]) into the canonical
//! installable bundle: a `.claude-plugin/plugin.json` manifest rooted over
//! `skills/<n>/SKILL.md`, `agents/<n>.md`, `artifacts/<id>.json`, `.mcp.json`,
//! and plugin-local scripts. It is the single owner of the bundle contract —
//! the gateway serve
//! path (manifest hashes *and* byte streaming), the CLI generator, and the
//! marketplace export all consume it rather than re-implementing the layout.
//!
//! ## Determinism
//!
//! The assembler is a pure function of the spec and the on-disk content: the
//! returned [`PluginBundle`] is keyed by sorted bundle-relative path and the
//! manifest's content-version hash is computed over every other file. Two calls
//! over identical inputs therefore produce byte-identical bundles, so the
//! manifest layer and the byte-serving layer agree without coordination.
//!
//! ## Boundary
//!
//! Platform-specific concerns (auth'd hook injection, per-skill tracking
//! tokens, download packaging) live with the *consumer*, never here. The
//! assembler only emits the host-facing contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, ManagedMcpServer, SkillEntry,
};
use systemprompt_models::bridge::plugin_bundle::{
    ManifestAuthor, PLUGIN_MANIFEST_RELPATH, PluginManifest, bundle_has_manifest,
};
use systemprompt_models::services::PluginConfig;

use crate::error::MarketplaceError;

mod agents;
mod artifacts;
mod mcp;
mod skills;

#[derive(Debug, Clone)]
pub struct BundleFile {
    pub bytes: Vec<u8>,
    pub executable: bool,
}

pub type PluginBundle = BTreeMap<String, BundleFile>;

#[derive(Debug)]
pub struct BundleContent<'a> {
    pub skills: &'a [SkillEntry],
    pub agents: &'a [AgentEntry],
    pub mcp_servers: &'a [ManagedMcpServer],
    pub disabled_mcp_servers: &'a BTreeSet<String>,
    /// First-class catalogue entities, selected many-to-many by plugin spec —
    /// not owned by any one skill.
    pub artifacts: &'a [ArtifactEntry],
    pub plugins_root: &'a Path,
}

const HOOKS_RELPATH: &str = "./hooks/hooks.json";

/// A spec whose references resolve to nothing still yields a manifest-only
/// bundle, not an error; callers must gate that with [`bundle_has_content`].
pub fn build_plugin_bundle(
    config: &PluginConfig,
    content: &BundleContent<'_>,
) -> Result<PluginBundle, MarketplaceError> {
    let mut bundle = PluginBundle::new();

    let agent_ids = agents::resolve_agents(config, content.agents);
    skills::append_skill_files(config, content, &agent_ids, &mut bundle);
    agents::append_agent_files(content.agents, &agent_ids, &mut bundle);
    artifacts::append_artifact_files(config, content, &mut bundle);
    mcp::append_mcp_file(
        config,
        content.mcp_servers,
        content.disabled_mcp_servers,
        &mut bundle,
    )?;
    append_script_files(config, content.plugins_root, &mut bundle)?;

    let version = content_version(&config.version, &bundle);
    let manifest = build_manifest(config, &version);
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    bundle.insert(
        PLUGIN_MANIFEST_RELPATH.to_owned(),
        BundleFile {
            bytes: json,
            executable: false,
        },
    );

    debug_assert!(bundle_has_manifest(bundle.keys().map(String::as_str)));
    Ok(bundle)
}

#[must_use]
pub fn bundle_has_content(bundle: &PluginBundle) -> bool {
    bundle.keys().any(|path| path != PLUGIN_MANIFEST_RELPATH)
}

fn build_manifest(config: &PluginConfig, version: &str) -> PluginManifest {
    PluginManifest {
        name: config.id.as_str().to_owned(),
        description: config.description.clone(),
        version: version.to_owned(),
        author: Some(ManifestAuthor {
            name: config.author.name.clone(),
            email: config.author.email.clone(),
        }),
        hooks: Some(HOOKS_RELPATH.to_owned()),
        keywords: config.keywords.clone(),
        installation_preference: None,
    }
}

fn content_version(base: &str, bundle: &PluginBundle) -> String {
    let mut hasher = Sha256::new();
    for (path, file) in bundle {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        hasher.update(&file.bytes);
        hasher.update(b"\0");
    }
    let digest = hasher.finalize();
    format!("{base}+{}", hex::encode(&digest[..4]))
}

fn append_script_files(
    config: &PluginConfig,
    plugins_root: &Path,
    bundle: &mut PluginBundle,
) -> Result<(), MarketplaceError> {
    for script in &config.scripts {
        if script.source == "generated:tracking" {
            continue;
        }
        let source = plugins_root.join(config.id.as_str()).join(&script.source);
        if !source.is_file() {
            continue;
        }
        let bytes = std::fs::read(&source).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        bundle.insert(
            format!("scripts/{}", script.name),
            BundleFile {
                bytes,
                executable: true,
            },
        );
    }
    Ok(())
}
