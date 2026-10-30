//! The declared hash and pipeline version recomputed from the database rows,
//! so a file-applied and a console-applied configuration agree byte for
//! byte.
//!
//! [`declared_hash_from_rows`] decodes every row through its typed
//! declaration and hashes the resulting entities with
//! `DeclaredState::from_entries`, exactly as the file side does.
//! [`current_pipeline_version`] composes that hash with the brain code
//! version and the model prompt versions and records the result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sqlx::PgConnection;
use systemprompt_identifiers::{FactKindId, PipelineVersion};
use systemprompt_traits::RepositoryError;

use super::entities::DeclaredBrain;
use crate::error::BrainResult;
use crate::models::PlaneRow;
use crate::repository::{
    ConfigDocRepository, InterestRepository, KindRepository, PipelineVersionRepository,
    TopicRepository, WorkflowRepository,
};
use crate::version::pipeline_inputs;

/// The raw rows of the five brain plane tables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaneRows {
    pub kinds: Vec<PlaneRow>,
    pub topics: Vec<PlaneRow>,
    pub interests: Vec<PlaneRow>,
    pub workflows: Vec<PlaneRow>,
    pub docs: Vec<PlaneRow>,
}

impl PlaneRows {
    pub async fn read(conn: &mut PgConnection) -> Result<Self, RepositoryError> {
        Ok(Self {
            kinds: KindRepository::list(&mut *conn).await?,
            topics: TopicRepository::list(&mut *conn).await?,
            interests: InterestRepository::list(&mut *conn).await?,
            workflows: WorkflowRepository::list(&mut *conn).await?,
            docs: ConfigDocRepository::list(&mut *conn).await?,
        })
    }
}

pub fn declared_hash_from_rows(rows: &PlaneRows) -> Result<String, serde_json::Error> {
    DeclaredBrain::from_rows(rows)?.declared_hash()
}

pub async fn load_declared_state(conn: &mut PgConnection) -> BrainResult<DeclaredBrain> {
    let rows = PlaneRows::read(conn).await?;
    Ok(DeclaredBrain::from_rows(&rows)?)
}

pub async fn current_pipeline_version(
    conn: &mut PgConnection,
    prompt_versions: BTreeMap<FactKindId, String>,
) -> BrainResult<PipelineVersion> {
    let rows = PlaneRows::read(&mut *conn).await?;
    let declared_hash = declared_hash_from_rows(&rows)?;
    let inputs = pipeline_inputs(declared_hash, prompt_versions);
    Ok(PipelineVersionRepository::record(conn, &inputs).await?)
}
