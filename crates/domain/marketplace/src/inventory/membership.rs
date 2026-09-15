//! Effective-time observations and immutable configured-resource bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ObservedMembership;
use super::catalog::invalid;
use crate::managed::{ManagedError, ManagedRepository, Result};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, UserId};

impl ManagedRepository {
    pub async fn inventory_membership(
        &self,
        owner: &UserId,
        id: &InventoryEntryId,
        at: DateTime<Utc>,
    ) -> Result<ObservedMembership> {
        if at > Utc::now() {
            return Ok(ObservedMembership::Unknown);
        }
        let row=sqlx::query!("SELECT effective_from,effective_until,record FROM managed_inventory_membership WHERE owner_id=$1 AND entry_id=$2 AND effective_from<=$3 AND (effective_until IS NULL OR effective_until>$3)",owner.as_str(),id.as_str(),at).fetch_optional(&self.pool).await?;
        row.map_or(Ok(ObservedMembership::Unknown), |row| {
            Ok(ObservedMembership::Known {
                effective_from: row.effective_from,
                effective_until: row.effective_until,
                entry: Box::new(serde_json::from_value(row.record)?),
            })
        })
    }

    pub async fn bind_inventory_resource(
        &self,
        owner: &UserId,
        actor: &UserId,
        entry: &InventoryEntryId,
        resource: &ManagedResourceId,
    ) -> Result<()> {
        let configured = self.inventory_entry(owner, entry).await?;
        if configured.configured_key.is_none() {
            return Err(invalid(
                "Only configured inventory entries accept explicit managed bindings",
            ));
        }
        let kind = sqlx::query_scalar!(
            "SELECT kind FROM managed_resources WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            resource.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        let supporting_binding = kind == "supporting"
            && !matches!(configured.kind.as_str(), "skill" | "plugin" | "marketplace");
        if kind != configured.kind && !supporting_binding {
            return Err(invalid("Inventory and managed resource kinds differ"));
        }
        sqlx::query!("INSERT INTO managed_inventory_bindings(owner_id,entry_id,resource_id,bound_by) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",owner.as_str(),entry.as_str(),resource.as_str(),actor.as_str()).execute(&self.pool).await?;
        let matched=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_inventory_bindings WHERE owner_id=$1 AND entry_id=$2 AND resource_id=$3)",owner.as_str(),entry.as_str(),resource.as_str()).fetch_one(&self.pool).await?.unwrap_or(false);
        if !matched {
            return Err(invalid(
                "Inventory identity already has a different immutable binding",
            ));
        }
        Ok(())
    }

    pub async fn inventory_coverage_at(
        &self,
        owner: &UserId,
        at: DateTime<Utc>,
    ) -> Result<super::InventoryCoverage> {
        let observed=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_inventory_observations WHERE owner_id=$1 AND observed_at<=$2)",owner.as_str(),at).fetch_one(&self.pool).await?.unwrap_or(false)&&at<=Utc::now();
        if !observed {
            let status = self.inventory_status(owner).await?;
            return Ok(super::InventoryCoverage {
                observation_available: false,
                known_total: 0,
                known_available: 0,
                unknown_membership: status.entries,
                as_of: at,
            });
        }
        let row=sqlx::query!(r#"SELECT COUNT(*) AS "known_total!",COUNT(*) FILTER(WHERE record->>'availability'='available') AS "known_available!" FROM managed_inventory_membership WHERE owner_id=$1 AND effective_from<=$2 AND (effective_until IS NULL OR effective_until>$2)"#,owner.as_str(),at).fetch_one(&self.pool).await?;
        let unknown=sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM managed_inventory_entries WHERE owner_id=$1 AND first_observed_at>$2"#,owner.as_str(),at).fetch_one(&self.pool).await?;
        Ok(super::InventoryCoverage {
            observation_available: true,
            known_total: row.known_total,
            known_available: row.known_available,
            unknown_membership: unknown,
            as_of: at,
        })
    }
}
