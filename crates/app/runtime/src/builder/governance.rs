//! Governance assembly: the governed extension set and the policy engine the
//! context is built with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Profile;
use systemprompt_traits::ConfigWatch;

use super::core_layer::{SchemaPolicy, init_extensions};
use crate::error::RuntimeResult;

pub(super) async fn init_governed_extensions(
    profile: &Profile,
    registry: Option<ExtensionRegistry>,
    schema: SchemaPolicy,
    migration_config: MigrationConfig,
    database: &Arc<systemprompt_database::Database>,
) -> RuntimeResult<(
    Arc<ExtensionRegistry>,
    systemprompt_database::SchemaInstallReport,
    ConfigWatch<systemprompt_governance::policy::GovernanceEngine>,
)> {
    let (registry, report) =
        init_extensions(profile, registry, schema, migration_config, database).await?;
    let governance = ConfigWatch::new(load_governance(profile, database).await?);
    Ok((registry, report, governance))
}

pub(crate) async fn load_governance(
    profile: &Profile,
    database: &systemprompt_database::DbPool,
) -> Result<
    systemprompt_governance::policy::GovernanceEngine,
    systemprompt_governance::policy::GovernanceEngineError,
> {
    systemprompt_governance::policy::GovernanceEngine::load(
        database.pool().as_ref(),
        Some(systemprompt_governance::policy::RateWindowRepository::new(
            database.write_pool().as_ref().clone(),
        )),
        profile.unapplied_plane_mode(),
    )
    .await
}
