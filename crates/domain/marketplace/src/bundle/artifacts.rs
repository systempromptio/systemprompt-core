//! Artifact component projection: selecting a plugin's artifacts from the
//! resolved catalogue and laying them out as `artifacts/manifest.json` (the
//! install records), `artifacts/<id>.html` (the page, verbatim) and
//! `artifacts/<id>.json` (the Cowork library record).
//!
//! Artifacts are first-class catalogue entities (`services/artifacts/<id>/`),
//! not children of any one skill: selection is many-to-many, so the same
//! artifact may ship in several plugins' bundles. They are emitted at the
//! bundle root alongside `skills/` and `agents/` to mirror that.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::cowork_artifact::{
    CoworkArtifactBundleManifest, CoworkArtifactBundleRecord, CoworkLibraryArtifactRecord,
};
use systemprompt_models::services::PluginConfig;

use crate::catalog::selects_artifact;
use crate::error::MarketplaceError;

use super::{BundleContent, BundleFile, PluginBundle};

pub(super) fn append_artifact_files(
    config: &PluginConfig,
    content: &BundleContent<'_>,
    bundle: &mut PluginBundle,
) -> Result<(), MarketplaceError> {
    let mut records = Vec::new();
    for artifact in content
        .artifacts
        .iter()
        .filter(|a| selects_artifact(config, &a.id))
    {
        let id = artifact.id.as_str();
        let record = CoworkLibraryArtifactRecord::from(artifact);
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| {
            MarketplaceError::Catalog(format!("artifact '{id}' record does not serialise: {e}"))
        })?;
        bundle.insert(format!("artifacts/{id}.json"), plain(bytes));
        bundle.insert(
            format!("artifacts/{id}.html"),
            plain(artifact.content.as_bytes().to_vec()),
        );
        records.push(CoworkArtifactBundleRecord::from(artifact));
    }
    if records.is_empty() {
        return Ok(());
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    let manifest = CoworkArtifactBundleManifest { artifacts: records };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| {
        MarketplaceError::Catalog(format!(
            "plugin '{}' artifact manifest does not serialise: {e}",
            config.id
        ))
    })?;
    bundle.insert("artifacts/manifest.json".to_owned(), plain(bytes));
    Ok(())
}

const fn plain(bytes: Vec<u8>) -> BundleFile {
    BundleFile {
        bytes,
        executable: false,
    }
}
