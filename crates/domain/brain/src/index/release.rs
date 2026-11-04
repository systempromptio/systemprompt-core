//! [`release_alias_owners`]: an erased, retired, staled or superseded
//! fact holds no alias. Its alias rows are deleted as a set and every
//! live fact that could still contribute one of them is reindexed through
//! the one alias path ([`super::reindex_facts`]), so the owner rule picks
//! the newest remaining contributor and an alias nobody live contributes
//! is simply gone. Deterministic: the same facts yield the same owner
//! whatever order the releases ran in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::FactStatus;
use systemprompt_traits::RepositoryError;

use super::reindex_facts;
use crate::repository::{AliasRepository, SynonymsRepository};

#[must_use]
pub const fn holds_aliases(status: FactStatus) -> bool {
    matches!(
        status,
        FactStatus::Candidate | FactStatus::Proposed | FactStatus::Confirmed | FactStatus::Disputed
    )
}

pub async fn release_alias_owners(
    conn: &mut PgConnection,
    owners: &[BrainFactId],
) -> Result<u64, RepositoryError> {
    if owners.is_empty() {
        return Ok(0);
    }
    let released = AliasRepository::delete_owned(&mut *conn, owners).await?;
    if released.is_empty() {
        return Ok(0);
    }
    let contributors = AliasRepository::list_contributors(&mut *conn, &released).await?;
    if contributors.is_empty() {
        return Ok(0);
    }
    let synonyms = SynonymsRepository::load(&mut *conn).await?;
    let report = reindex_facts(conn, &contributors, &synonyms).await?;
    Ok(report.aliases)
}
