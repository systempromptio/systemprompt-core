//! The `governance` config plane: `services/governance/config.yaml`
//! projected into `governance_config`.
//!
//! The policy chain is read from the database at runtime
//! ([`GovernanceEngine::load`](super::GovernanceEngine::load)) and rebuilt on
//! every node when the plane's `config:governance` notification arrives, so
//! an apply or a console edit changes enforcement without a restart. The file
//! reaches the database only through `config apply` or `/admin/sync`; boot
//! never writes it. An empty table is the documented warn-only fallback
//! chain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod canonical;
mod store;

use std::path::Path;

use async_trait::async_trait;
use systemprompt_database::PlaneTransaction;
use systemprompt_extension::prelude::{
    ApplyOutcome, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, DeclaredState, PlaneContext,
    PlaneSnapshot, PlaneTx, ResolvedEntities,
};

pub use canonical::{CHAIN_ENTITY, POLICY_PREFIX, config_from_entities, declared_entities};
pub use store::list_governance_config;

use super::config::GovernanceConfigError;

pub const GOVERNANCE_FILE: &str = "governance/config.yaml";
pub const GOVERNANCE_PLANE: &str = "governance";

#[derive(Debug, Clone, Copy, Default)]
pub struct GovernancePlane;

fn invalid(error: &GovernanceConfigError) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: GOVERNANCE_PLANE.to_owned(),
        path: GOVERNANCE_FILE.to_owned(),
        message: error.to_string(),
    }
}

fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: GOVERNANCE_PLANE.to_owned(),
        source: Box::new(error),
    }
}

#[async_trait]
impl ConfigPlane for GovernancePlane {
    fn name(&self) -> &'static str {
        GOVERNANCE_PLANE
    }

    fn declared_path(&self) -> &'static str {
        GOVERNANCE_FILE
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        let text = match std::fs::read_to_string(services_root.join(GOVERNANCE_FILE)) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(ConfigPlaneError::Unreadable {
                    plane: GOVERNANCE_PLANE.to_owned(),
                    path: GOVERNANCE_FILE.to_owned(),
                    source: Box::new(error),
                });
            },
        };
        let entries = declared_entities(&text).map_err(|e| invalid(&e))?;
        Ok(Some(DeclaredState::from_entries(entries)))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        store::list_on(tx).await.map_err(backend)
    }

    async fn write(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (entity, resolved) in entities {
            if let Some(value) = &resolved.value {
                store::upsert_on(tx, entity, value, &resolved.source.to_string())
                    .await
                    .map_err(backend)?;
                outcome.updated += 1;
            } else {
                store::delete_on(tx, entity).await.map_err(backend)?;
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
        canonical::render_document(entities).map_err(|e| invalid(&e))
    }
}
