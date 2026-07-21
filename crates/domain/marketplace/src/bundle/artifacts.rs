//! Artifact component projection: selecting a plugin's artifacts from the
//! resolved catalogue and laying them out as `artifacts/<id>.json`.
//!
//! Artifacts are first-class catalogue entities (`services/artifacts/<id>/`),
//! not children of any one skill: selection is many-to-many, so the same
//! artifact may ship in several plugins' bundles. They are emitted at the
//! bundle root alongside `skills/` and `agents/` to mirror that.
//!
//! The record shape mirrors Cowork's `create_artifact` input and must stay in
//! step with the bridge's staging writer (`LibraryArtifactRecord` in
//! `bin/bridge/src/integration/cowork_artifacts/sink.rs`), so a consumer can
//! read either source with one parser.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::bridge::manifest::ArtifactEntry;
use systemprompt_models::services::PluginConfig;

use crate::catalog::selects_artifact;

use super::{BundleContent, BundleFile, PluginBundle};

/// Field names track Cowork's native library-entry shape.
#[derive(Debug, Serialize)]
struct BundledArtifactRecord<'a> {
    id: &'a str,
    name: &'a str,
    description: &'a str,
    version: &'a str,
    content: &'a str,
    #[serde(rename = "isStarred")]
    is_starred: bool,
    #[serde(rename = "mcpTools")]
    mcp_tools: &'a [String],
}

impl<'a> From<&'a ArtifactEntry> for BundledArtifactRecord<'a> {
    fn from(a: &'a ArtifactEntry) -> Self {
        Self {
            id: a.id.as_str(),
            name: &a.name,
            description: &a.description,
            version: &a.version,
            content: &a.content,
            is_starred: a.starred,
            mcp_tools: &a.mcp_tools,
        }
    }
}

/// A record that fails to serialise is dropped with a warning rather than
/// failing the whole bundle, mirroring the catalogue's fail-closed drops.
pub(super) fn append_artifact_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    bundle: &mut PluginBundle,
) {
    for artifact in content
        .artifacts
        .iter()
        .filter(|a| selects_artifact(config, a.id.as_str()))
    {
        let record = BundledArtifactRecord::from(artifact);
        match serde_json::to_vec_pretty(&record) {
            Ok(bytes) => {
                bundle.insert(
                    format!("artifacts/{}.json", artifact.id.as_str()),
                    BundleFile {
                        bytes,
                        executable: false,
                    },
                );
            },
            Err(e) => {
                tracing::warn!(
                    artifact_id = %artifact.id.as_str(),
                    error = %e,
                    "bundle: failed to serialise artifact record; skipping"
                );
            },
        }
    }
}
