//! Artifact component projection: selecting a plugin's artifacts from the
//! resolved catalogue and laying them out as `artifacts/<id>.json`.
//!
//! Artifacts are first-class catalogue entities (`services/artifacts/<id>/`),
//! not children of any one skill: selection is many-to-many, so the same
//! artifact may ship in several plugins' bundles. They are emitted at the
//! bundle root alongside `skills/` and `agents/` to mirror that.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::manifest::CoworkLibraryArtifactRecord;
use systemprompt_models::services::PluginConfig;

use crate::catalog::selects_artifact;

use super::{BundleContent, BundleFile, PluginBundle};

pub(super) fn append_artifact_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    bundle: &mut PluginBundle,
) {
    for artifact in content
        .artifacts
        .iter()
        .filter(|a| selects_artifact(config, &a.id))
    {
        let record = CoworkLibraryArtifactRecord::from(artifact);
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
