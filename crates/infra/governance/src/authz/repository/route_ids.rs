//! Moving access rules from a generated legacy route id onto the declared id
//! that replaced it (`config migrate --data`, credential.md §8 Phase 3.4).
//!
//! For each `(legacy, declared)` pair the declared route's entity row is
//! created from the legacy one if missing, a legacy rule the declared route
//! already carries is dropped, the rest move across, and the legacy entity
//! row is deleted. All pairs run in one transaction under change context
//! actor `cli`, origin `migration`, so the rows are attributed and a failure
//! leaves every rule where it was. A pair whose legacy id has no rows is a
//! no-op, which makes a rerun safe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::with_change_context;
use systemprompt_traits::{ChangeActor, ChangeSource};

use super::AccessControlRepository;
use crate::authz::error::AuthzResult;

const REASON: &str = "config migrate --data: legacy route id rewritten to its declared id";

impl AccessControlRepository {
    pub async fn update_route_ids(&self, renames: &[(&str, &str)]) -> AuthzResult<u64> {
        if renames.is_empty() {
            return Ok(0);
        }
        let mut tx = self.write_pool.begin().await?;
        with_change_context(
            &mut tx,
            &ChangeActor::Cli,
            Some(REASON),
            &ChangeSource::Migration,
        )
        .await?;
        let mut moved = 0;
        for (legacy, declared) in renames {
            sqlx::query!(
                r"INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
                  SELECT entity_type, $2, default_included, 'migration'
                    FROM access_control_entities
                   WHERE entity_type = 'gateway_route' AND entity_id = $1
                  ON CONFLICT (entity_type, entity_id) DO NOTHING",
                legacy,
                declared,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                r"DELETE FROM access_control_rules old
                   WHERE old.entity_type = 'gateway_route' AND old.entity_id = $1
                     AND EXISTS (SELECT 1 FROM access_control_rules kept
                                  WHERE kept.entity_type = 'gateway_route'
                                    AND kept.entity_id = $2
                                    AND kept.rule_type = old.rule_type
                                    AND kept.rule_value = old.rule_value)",
                legacy,
                declared,
            )
            .execute(&mut *tx)
            .await?;
            moved += sqlx::query!(
                r"UPDATE access_control_rules SET entity_id = $2, updated_at = NOW()
                   WHERE entity_type = 'gateway_route' AND entity_id = $1",
                legacy,
                declared,
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();
            sqlx::query!(
                "DELETE FROM access_control_entities WHERE entity_type = 'gateway_route' AND \
                 entity_id = $1",
                legacy,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(moved)
    }
}
