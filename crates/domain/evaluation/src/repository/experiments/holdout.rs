//! Consume semantic holdout content atomically, independent of revision
//! aliases.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use crate::Result;
use crate::experiments::{ExperimentSpec, conflict};
use systemprompt_identifiers::{EvalExperimentId, UserId};

pub(super) async fn consume(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserId,
    experiment: &EvalExperimentId,
    spec: &ExperimentSpec,
) -> Result<()> {
    if !spec.claim_independent_improvement {
        return Ok(());
    }
    let ids = spec
        .cases
        .iter()
        .map(|case| case.as_str().to_owned())
        .collect::<Vec<_>>();
    let cases=sqlx::query!("SELECT id,md5(((content->'content')-'partition')::text) AS digest FROM eval_resource_revisions WHERE owner_id=$1 AND id=ANY($2) AND content->'content'->>'partition'='holdout'",owner.as_str(),&ids).fetch_all(&mut **tx).await?;
    let mut seen = std::collections::BTreeSet::new();
    for case in cases {
        let digest = case
            .digest
            .ok_or_else(|| conflict("Holdout content digest unavailable"))?;
        if !seen.insert(digest.clone()) {
            return Err(conflict(
                "Holdout case content must be independently distinct",
            ));
        }
        let exposed=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_resource_revisions r JOIN eval_executions x ON x.case_revision_id=r.id JOIN eval_experiments e ON e.id=x.experiment_id WHERE r.owner_id=$1 AND e.id<>$3 AND md5(((r.content->'content')-'partition')::text)=$2) OR EXISTS(SELECT 1 FROM eval_resource_revisions r WHERE r.owner_id=$1 AND r.id=ANY($4) AND r.content->'content'->>'partition'='development' AND md5(((r.content->'content')-'partition')::text)=$2)",owner.as_str(),digest,experiment.as_str(),&ids).fetch_one(&mut **tx).await?.unwrap_or(true);
        if exposed {
            return Err(conflict(
                "Holdout content was already exposed in an execution or development partition",
            ));
        }
        let inserted=sqlx::query!("INSERT INTO eval_holdout_content_consumption(owner_id,content_digest,experiment_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING RETURNING content_digest",owner.as_str(),digest,experiment.as_str()).fetch_optional(&mut **tx).await?;
        if inserted.is_none() {
            return Err(conflict("Fresh independent holdout content is required"));
        }
    }
    Ok(())
}
