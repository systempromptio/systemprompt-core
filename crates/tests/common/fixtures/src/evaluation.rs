//! Real-repository seams for the evaluation domain: the AI request trace,
//! the users session provider and the managed-revision ownership lookup,
//! built the way the application composition root builds them.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_evaluation::repository::experiments::{EvaluationRepositories, EvaluationSeams};

pub fn fixture_evaluation_seams(pool: &DbPool) -> Result<EvaluationSeams> {
    Ok(EvaluationSeams {
        trace: Arc::new(systemprompt_ai::repository::AiRequestRepository::new(pool)?),
        sessions: Arc::new(systemprompt_users::UsersAiSessionProvider::from_repository(
            systemprompt_users::SessionRepository::new(pool)?,
        )),
        managed_revisions: Arc::new(systemprompt_marketplace::managed::ManagedRepository::new(
            pool,
        )?),
    })
}

pub fn fixture_evaluation_repositories(pool: &DbPool) -> Result<EvaluationRepositories> {
    Ok(EvaluationRepositories::new(
        pool,
        fixture_evaluation_seams(pool)?,
    )?)
}

// A managed skill resource with one retained revision owned by `owner`, the
// baseline a campaign policy must name.
pub async fn seed_managed_baseline(
    pool: &DbPool,
    owner: &systemprompt_identifiers::UserId,
    key: &str,
) -> Result<(
    systemprompt_identifiers::ManagedResourceId,
    systemprompt_identifiers::ResourceRevisionId,
)> {
    use systemprompt_marketplace::managed::{
        AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, ResourceKind,
        RevisionFiles, SnapshotProvenance, SourceSpec,
    };
    let managed = ManagedRepository::new(pool)?;
    let source = managed
        .register_source(owner, key, &SourceSpec::Managed)
        .await?;
    let snapshot = managed
        .capture_snapshot(
            owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(key.as_bytes()),
                importer_version: "test-fixture".to_owned(),
            },
        )
        .await?;
    let resource = managed
        .bind_resource(
            owner,
            &NewResource {
                source_id: source,
                upstream_key: key.to_owned(),
                kind: ResourceKind::Skill,
                resource_key: key.to_owned(),
            },
        )
        .await?;
    let files = RevisionFiles(std::collections::BTreeMap::from([(
        "index.md".to_owned(),
        AssetFile {
            bytes: b"# baseline".to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    )]));
    let revision = managed
        .create_revision(
            owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files,
                dependencies: std::collections::BTreeMap::new(),
                rationale: "baseline".to_owned(),
            },
        )
        .await?;
    Ok((resource, revision))
}
