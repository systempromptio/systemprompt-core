//! The `access_control` config plane: `services/access-control/rules.yaml`
//! projected into `access_control_entities` and `access_control_rules`.
//!
//! Each entity block is one plane entity keyed `<kind>/<id>`, its value the
//! [`CanonicalEntity`] JSON. An `owner: bundle:<name>` block is applied with
//! that bundle as its origin, every other one with `code:<rules file>`. Globs
//! and `model` / `provider` entitlements are stored as written; the resolver
//! matches them at decision time, so a route or model added later is covered
//! without another apply. A tree that still carries the retired
//! `access-control/roles.yaml` is refused with the `config migrate` hint.
//! Boot never writes this plane: `config apply` and
//! `/admin/sync` do, through the per-entity three-way merge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod store;

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use systemprompt_extension::prelude::{
    ApplyOutcome, ChangeSource, ConfigPlane, ConfigPlaneError, ConfigPlaneResult, DeclaredEntities,
    DeclaredEntity, DeclaredState, PlaneContext, PlaneSnapshot, PlaneTx, ResolvedEntities,
};

use systemprompt_database::DbPool;

use crate::authz::error::{AuthzError, AuthzResult};
use crate::authz::rules_file::{
    CanonicalEntity, DeclaredAccess, EntityTarget, RETIRED_ROLES_FILE, RULES_FILE,
    declared_entities, read_rules_doc, render_rules_doc,
};
use store::PlaneTransaction;

pub const ACCESS_CONTROL_PLANE: &str = "access_control";

pub async fn render_current_rules(db: &DbPool) -> AuthzResult<String> {
    let mut tx = db.pool().begin().await?;
    let entities = store::list_declared_access(&mut tx).await?;
    tx.rollback().await?;
    render_rules_doc(&entities, &BTreeMap::new())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AccessControlPlane;

impl AccessControlPlane {
    fn declared(
        services_root: &Path,
    ) -> ConfigPlaneResult<Option<BTreeMap<String, DeclaredAccess>>> {
        if services_root.join(RETIRED_ROLES_FILE).exists() {
            return Err(ConfigPlaneError::Invalid {
                plane: ACCESS_CONTROL_PLANE.to_owned(),
                path: RETIRED_ROLES_FILE.to_owned(),
                message: format!(
                    "{RETIRED_ROLES_FILE} is retired; run `systemprompt admin config migrate` to \
                     fold it into {RULES_FILE}"
                ),
            });
        }
        let Some(doc) = read_rules_doc(services_root).map_err(|e| invalid(&e))? else {
            return Ok(None);
        };
        declared_entities(&doc).map(Some).map_err(|e| invalid(&e))
    }
}

fn invalid(error: &AuthzError) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: ACCESS_CONTROL_PLANE.to_owned(),
        path: RULES_FILE.to_owned(),
        message: error.to_string(),
    }
}

fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: ACCESS_CONTROL_PLANE.to_owned(),
        source: Box::new(error),
    }
}

fn canonical_values(
    entities: BTreeMap<String, DeclaredAccess>,
) -> ConfigPlaneResult<PlaneSnapshot> {
    entities
        .into_iter()
        .map(|(key, access)| Ok((key, access.canonical.to_value().map_err(|e| invalid(&e))?)))
        .collect()
}

#[async_trait]
impl ConfigPlane for AccessControlPlane {
    fn name(&self) -> &'static str {
        ACCESS_CONTROL_PLANE
    }

    fn declared_path(&self) -> &'static str {
        RULES_FILE
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        let Some(entities) = Self::declared(services_root)? else {
            return Ok(None);
        };
        Ok(Some(DeclaredState::from_entries(canonical_values(
            entities,
        )?)))
    }

    fn sources(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredEntities>> {
        let Some(entities) = Self::declared(services_root)? else {
            return Ok(None);
        };
        entities
            .into_iter()
            .map(|(key, access)| {
                let origin = access.owner_bundle.clone().map_or_else(
                    || ChangeSource::Code(RULES_FILE.to_owned()),
                    ChangeSource::Bundle,
                );
                let value = access.canonical.to_value().map_err(|e| invalid(&e))?;
                Ok((key, DeclaredEntity::new(value, origin)))
            })
            .collect::<ConfigPlaneResult<DeclaredEntities>>()
            .map(Some)
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let entities = store::list_declared_access(tx).await.map_err(backend)?;
        entities
            .into_iter()
            .map(|(key, canonical)| Ok((key, canonical.to_value().map_err(|e| invalid(&e))?)))
            .collect()
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
            let target: EntityTarget = key.parse().map_err(|e| invalid(&e))?;
            if let Some(value) = &resolved.value {
                let canonical = CanonicalEntity::from_value(value).map_err(|e| invalid(&e))?;
                store::set_entity(tx, &target, &canonical, &resolved.source.to_string())
                    .await
                    .map_err(backend)?;
                outcome.updated += 1;
            } else {
                store::clear_entity(tx, &target).await.map_err(backend)?;
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
        let canonical = entities
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    CanonicalEntity::from_value(value).map_err(|e| invalid(&e))?,
                ))
            })
            .collect::<ConfigPlaneResult<BTreeMap<_, _>>>()?;
        render_rules_doc(&canonical, &BTreeMap::new()).map_err(|e| invalid(&e))
    }
}
