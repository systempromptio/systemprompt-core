//! Moving access rules from a legacy route id onto the `model/<glob>` entity
//! that replaced it (`config migrate --data`, credential.md §8 Phase 3.4).
//!
//! A request no declared route matches is governed by the `model` and
//! `provider` entities, so the rules 0.72 attached to a routing-only route, or
//! to the catalog route id, belong on the model glob that route matched. For
//! each `(legacy route, model)` pair the model's entity row is created from
//! the legacy one if missing, a legacy rule the model already carries is
//! dropped, the rest move across, and the legacy entity row is deleted. All
//! pairs run in one transaction under change context actor `cli`, origin
//! `migration`, so the rows are attributed and a failure leaves every rule
//! where it was. A pair whose legacy id has no rows is a no-op, which makes a
//! rerun safe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::with_change_context;
use systemprompt_traits::{ChangeActor, ChangeSource};

use super::AccessControlRepository;
use crate::authz::error::AuthzResult;

const REASON: &str = "config migrate --data: legacy route rules moved onto their model entity";

impl AccessControlRepository {
    pub async fn move_route_rules_to_models(&self, pairs: &[(&str, &str)]) -> AuthzResult<u64> {
        if pairs.is_empty() {
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
        for (legacy, model) in pairs {
            sqlx::query!(
                r"INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
                  SELECT 'model', $2, default_included, 'migration'
                    FROM access_control_entities
                   WHERE entity_type = 'gateway_route' AND entity_id = $1
                  ON CONFLICT (entity_type, entity_id) DO NOTHING",
                legacy,
                model,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                r"DELETE FROM access_control_rules old
                   WHERE old.entity_type = 'gateway_route' AND old.entity_id = $1
                     AND EXISTS (SELECT 1 FROM access_control_rules kept
                                  WHERE kept.entity_type = 'model'
                                    AND kept.entity_id = $2
                                    AND kept.rule_type = old.rule_type
                                    AND kept.rule_value = old.rule_value)",
                legacy,
                model,
            )
            .execute(&mut *tx)
            .await?;
            moved += sqlx::query!(
                r"UPDATE access_control_rules SET entity_type = 'model', entity_id = $2, updated_at = NOW()
                   WHERE entity_type = 'gateway_route' AND entity_id = $1",
                legacy,
                model,
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
