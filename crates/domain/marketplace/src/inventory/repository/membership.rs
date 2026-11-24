//! Immutable bindings of configured inventory entries to managed resources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::inventory::catalog::invalid;
use crate::managed::{ManagedError, ManagedRepository, ResourceKind, Result};
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, UserId};

impl ManagedRepository {
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
            r#"SELECT kind AS "kind: ResourceKind" FROM managed_resources WHERE owner_id=$1 AND id=$2"#,
            owner as _,
            resource as _
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        let supporting_binding = kind == ResourceKind::Supporting
            && !matches!(configured.kind.as_str(), "skill" | "plugin" | "marketplace");
        if kind.as_str() != configured.kind && !supporting_binding {
            return Err(invalid("Inventory and managed resource kinds differ"));
        }
        sqlx::query!("INSERT INTO managed_inventory_bindings(owner_id,entry_id,resource_id,bound_by) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",owner as _,entry as _,resource as _,actor as _).execute(&self.pool).await?;
        let matched=sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_inventory_bindings WHERE owner_id=$1 AND entry_id=$2 AND resource_id=$3)",owner as _,entry as _,resource as _).fetch_one(&self.pool).await?.unwrap_or(false);
        if !matched {
            return Err(invalid(
                "Inventory identity already has a different immutable binding",
            ));
        }
        Ok(())
    }
}
