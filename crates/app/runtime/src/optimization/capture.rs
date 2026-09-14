//! Capture only configured authoring roots; clients cannot request arbitrary
//! server files through source specifications.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{OptimizationError, SkillOptimizationOrchestrator};
use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_marketplace::managed::{ImportedSkills, ManagedError, SourceSpec, capture_skills};

impl SkillOptimizationOrchestrator {
    pub async fn capture_authoring_skills(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
        configured_root: &std::path::Path,
        skills: Vec<String>,
    ) -> Result<ImportedSkills, OptimizationError> {
        let captured = self
            .capture_authoring_input(owner, source, configured_root, skills)
            .await?;
        Ok(self
            .managed
            .import_skills(owner, source, &captured, None)
            .await?)
    }
    pub async fn capture_authoring_input(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
        configured_root: &std::path::Path,
        skills: Vec<String>,
    ) -> Result<systemprompt_marketplace::managed::CapturedSkills, OptimizationError> {
        let SourceSpec::LocalTree { root } = self.managed.get_source(owner, source).await? else {
            return Err(OptimizationError::Source("Authoring capture requires a local-tree source; synchronize Git sources separately".to_owned()));
        };
        let expected = std::fs::canonicalize(configured_root).map_err(ManagedError::Io)?;
        let actual = std::fs::canonicalize(root).map_err(ManagedError::Io)?;
        if expected != actual {
            return Err(OptimizationError::Source(
                "Source root is not the configured authoring root".to_owned(),
            ));
        }
        let captured = tokio::task::spawn_blocking(move || capture_skills(&actual, &skills))
            .await
            .map_err(|error| {
                OptimizationError::Source(format!("Authoring capture task failed: {error}"))
            })??;
        Ok(captured)
    }
}
