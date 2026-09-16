//! Capture only configured authoring roots; clients cannot request arbitrary
//! server files through source specifications.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OrchestrationError;
use systemprompt_identifiers::{ManagedSourceId, UserId};
use systemprompt_marketplace::managed::{
    CapturedSkills, ManagedError, ManagedRepository, SourceSpec, capture_skills,
};

pub async fn capture_authoring_input(
    managed: &ManagedRepository,
    owner: &UserId,
    source: &ManagedSourceId,
    configured_root: &std::path::Path,
    skills: Vec<String>,
) -> Result<CapturedSkills, OrchestrationError> {
    let SourceSpec::LocalTree { root } = managed.get_source(owner, source).await? else {
        return Err(OrchestrationError::Source(
            "Authoring capture requires a local-tree source; synchronize Git sources separately"
                .to_owned(),
        ));
    };
    let expected = std::fs::canonicalize(configured_root).map_err(ManagedError::Io)?;
    let actual = std::fs::canonicalize(root).map_err(ManagedError::Io)?;
    if expected != actual {
        return Err(OrchestrationError::Source(
            "Source root is not the configured authoring root".to_owned(),
        ));
    }
    let captured = tokio::task::spawn_blocking(move || capture_skills(&actual, &skills))
        .await
        .map_err(|error| {
            OrchestrationError::Source(format!("Authoring capture task failed: {error}"))
        })??;
    Ok(captured)
}
