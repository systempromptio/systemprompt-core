//! The `brain` config plane.
//!
//! `services/brain/` holds `topics.yaml`, `filters.yaml`, `importance.yaml`,
//! `retrieval.yaml` and one declaration per file under `kinds/`,
//! `interests/` and `profiles/`. Together they parse into a
//! [`DeclaredBrain`], are validated as one `BrainDocs`, and project into
//! `brain_fact_kinds`, `brain_topics`, `brain_interests`, `brain_workflows`
//! and `brain_config_docs`.
//!
//! Entities are keyed `kind/<id>`, `topic/<id>`, `interest/<id>`,
//! `workflow/<id>` and `doc/{filters,importance,retrieval}`; each value is
//! the declaration's canonical JSON and each declared entity's origin is the
//! file that declares it. The database side decodes every row through the
//! same typed declaration, so a file-applied and a console-applied
//! configuration canonicalise, and hash ([`declared_hash_from_rows`]),
//! identically. No `services/brain/` directory leaves the database
//! authoritative.
//!
//! [`load_declared_state`] reads the rows into a typed [`DeclaredBrain`] and
//! [`current_pipeline_version`] records the pipeline version they imply.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entities;
mod files;
mod hash;
mod write;

use std::path::Path;

use async_trait::async_trait;
use systemprompt_database::PlaneTransaction;
use systemprompt_extension::prelude::{
    ApplyOutcome, ChangeSource, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, DeclaredEntities,
    DeclaredEntity, DeclaredState, PlaneContext, PlaneSnapshot, PlaneTx, ResolvedEntities,
};
use systemprompt_manifest::services::brain::BRAIN_DIR;

pub use entities::DeclaredBrain;
pub use files::render_files;
pub use hash::{PlaneRows, current_pipeline_version, declared_hash_from_rows, load_declared_state};

pub const BRAIN_PLANE: &str = "brain";

/// The brain declarations as a config plane.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainPlane;

pub(crate) fn unreadable(
    path: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> ConfigPlaneError {
    ConfigPlaneError::Unreadable {
        plane: BRAIN_PLANE.to_owned(),
        path: path.to_owned(),
        source: Box::new(error),
    }
}

pub(crate) fn invalid(path: &str, message: &dyn std::fmt::Display) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: BRAIN_PLANE.to_owned(),
        path: path.to_owned(),
        message: message.to_string(),
    }
}

pub(crate) fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: BRAIN_PLANE.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl ConfigPlane for BrainPlane {
    fn name(&self) -> &'static str {
        BRAIN_PLANE
    }

    fn declared_path(&self) -> &'static str {
        BRAIN_DIR
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(files::declared(services_root)?.map(|entities| {
            DeclaredState::from_entries(entities.into_iter().map(|(k, (v, _))| (k, v)).collect())
        }))
    }

    fn sources(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredEntities>> {
        Ok(files::declared(services_root)?.map(|entities| {
            entities
                .into_iter()
                .map(|(key, (value, file))| {
                    (key, DeclaredEntity::new(value, ChangeSource::Code(file)))
                })
                .collect()
        }))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let rows = PlaneRows::read(tx).await.map_err(backend)?;
        DeclaredBrain::from_rows(&rows)
            .and_then(|brain| brain.entities())
            .map_err(|e| invalid(BRAIN_DIR, &e))
    }

    async fn write(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (key, resolved) in entities {
            write::write_entity(tx, key, resolved).await?;
            if resolved.value.is_some() {
                outcome.updated += 1;
            } else {
                outcome.deleted += 1;
            }
        }
        Ok(outcome)
    }

    async fn export(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        let current = self.read_current(ctx, tx).await?;
        if current.is_empty() {
            return Ok(None);
        }
        self.render_declarations(&current).map(Some)
    }

    fn render_declarations(&self, entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
        let brain = DeclaredBrain::from_entities(entities)?;
        let rendered = render_files(&brain)?;
        serde_yaml::to_string(&rendered).map_err(|e| invalid(BRAIN_DIR, &e))
    }
}
