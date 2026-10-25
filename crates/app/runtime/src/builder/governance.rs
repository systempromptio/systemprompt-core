use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_traits::ConfigWatch;

use super::core_layer::{SchemaPolicy, init_extensions};
use crate::error::RuntimeResult;

pub(super) async fn init_governed_extensions(
    registry: Option<ExtensionRegistry>,
    schema: SchemaPolicy,
    migration_config: MigrationConfig,
    database: &Arc<systemprompt_database::Database>,
) -> RuntimeResult<(
    Arc<ExtensionRegistry>,
    systemprompt_database::SchemaInstallReport,
    ConfigWatch<systemprompt_governance::policy::GovernanceEngine>,
)> {
    let (registry, report) = init_extensions(registry, schema, migration_config, database).await?;
    let governance = ConfigWatch::new(load_governance(database).await?);
    Ok((registry, report, governance))
}

pub(crate) async fn load_governance(
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
        systemprompt_config::ProfileBootstrap::get().map_or_else(
            |_| systemprompt_manifest::profile::UnappliedPlaneMode::default(),
            systemprompt_manifest::Profile::unapplied_plane_mode,
        ),
    )
    .await
}
