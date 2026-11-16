//! The policy engine the context is built with, loaded over the kernel-built
//! [`PolicyCatalog`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::policy::PolicyCatalog;
use systemprompt_manifest::Profile;

pub(crate) async fn load_governance(
    profile: &Profile,
    database: &systemprompt_database::DbPool,
    catalog: &PolicyCatalog,
) -> Result<
    systemprompt_governance::policy::GovernanceEngine,
    systemprompt_governance::policy::GovernanceEngineError,
> {
    systemprompt_governance::policy::GovernanceEngine::load(
        database.pool().as_ref(),
        catalog,
        Some(systemprompt_governance::policy::RateWindowRepository::new(
            database.write_pool().as_ref().clone(),
        )),
        profile.unapplied_plane_mode(),
    )
    .await
}
