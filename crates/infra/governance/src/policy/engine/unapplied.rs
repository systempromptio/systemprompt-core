//! The engine a node runs while the `governance` plane was never applied.
//!
//! No chain runs. The profile's `governance.unapplied_planes` decides the
//! verdict the chain walk returns (see `chain`): `deny` refuses every call,
//! `warn` passes it as a `Warn` carrying the same reason. Either way the boot
//! log says so and names the apply command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_manifest::profile::UnappliedPlaneMode;

use super::{GovernanceEngine, GovernanceEngineError};
use crate::policy::config::GovernanceConfig;
use crate::policy::plane::GOVERNANCE_PLANE;
use crate::policy::rate_window::RateWindowRepository;

impl GovernanceEngine {
    pub(super) fn unapplied(
        mode: UnappliedPlaneMode,
        rate_windows: Option<RateWindowRepository>,
    ) -> Result<Self, GovernanceEngineError> {
        if mode.denies() {
            tracing::error!(
                "governance plane not applied (unapplied_planes: deny): every governed request \
                 is denied until `systemprompt admin config apply` writes it"
            );
        } else {
            tracing::warn!(
                "governance plane not applied (unapplied_planes: warn): governed requests pass \
                 UNGOVERNED until `systemprompt admin config apply` writes it"
            );
        }
        let mut empty = GovernanceConfig::defaults();
        empty.policies.clear();
        let mut engine = Self::from_config(&empty, rate_windows)?;
        engine.unapplied = Some(mode);
        Ok(engine)
    }
}

pub(super) async fn plane_applied(pool: &PgPool) -> bool {
    match systemprompt_database::find_config_plane_applied(pool, GOVERNANCE_PLANE).await {
        Ok(applied) => applied,
        Err(error) => {
            tracing::error!(%error, "governance plane apply state unreadable; treating it as not applied");
            false
        },
    }
}
