//! Compare-and-set authoring observation heads and incoming reconciliations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::inventory::catalog::invalid;
use crate::managed::{ManagedRepository, ReconciliationStatus, Result};
use systemprompt_identifiers::{
    InventoryEntryId, ManagedReconciliationId, ManagedResourceId, ResourceRevisionId, UserId,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct IncomingRevision<'a> {
    pub(crate) resource: &'a ManagedResourceId,
    pub(crate) base: Option<&'a ResourceRevisionId>,
    pub(crate) candidate: Option<&'a ResourceRevisionId>,
    pub(crate) incoming: &'a ResourceRevisionId,
}

impl ManagedRepository {
    pub(crate) async fn latest_inventory_revision(
        &self,
        owner: &UserId,
        resource: &ManagedResourceId,
    ) -> Result<Option<ResourceRevisionId>> {
        Ok(sqlx::query_scalar!("SELECT id FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 ORDER BY created_at DESC,id DESC LIMIT 1",owner as _,resource as _).fetch_optional(&self.pool).await?.map(ResourceRevisionId::new))
    }

    pub(crate) async fn reconcile_inventory_incoming(
        &self,
        owner: &UserId,
        revision: &IncomingRevision<'_>,
    ) -> Result<Option<ManagedReconciliationId>> {
        let (Some(base), Some(candidate)) = (revision.base, revision.candidate) else {
            return Ok(None);
        };
        if base == candidate || candidate == revision.incoming {
            return Ok(None);
        }
        let reconciliation = self
            .begin_reconciliation(
                owner,
                &crate::managed::ReconciliationRequest {
                    resource_id: revision.resource.clone(),
                    upstream_base_revision_id: base.clone(),
                    managed_candidate_revision_id: candidate.clone(),
                    incoming_revision_id: revision.incoming.clone(),
                },
            )
            .await?;
        Ok(Some(reconciliation.id))
    }

    pub(in crate::inventory) async fn inventory_authoring_head(
        &self,
        owner: &UserId,
        entry: &InventoryEntryId,
    ) -> Result<Option<ResourceRevisionId>> {
        Ok(sqlx::query_scalar!("SELECT revision_id FROM managed_inventory_authoring_heads WHERE owner_id=$1 AND entry_id=$2",owner as _,entry as _).fetch_optional(&self.pool).await?.map(ResourceRevisionId::new))
    }

    pub(in crate::inventory) async fn advance_inventory_head(
        &self,
        owner: &UserId,
        entry: &InventoryEntryId,
        expected: Option<&ResourceRevisionId>,
        revision: &ResourceRevisionId,
    ) -> Result<()> {
        let updated=sqlx::query!("INSERT INTO managed_inventory_authoring_heads(owner_id,entry_id,revision_id) SELECT $1,$2,$3 WHERE $4::text IS NULL ON CONFLICT(owner_id,entry_id) DO NOTHING",owner as _,entry as _,revision as _,expected as _).execute(&self.pool).await?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        let changed=sqlx::query!("UPDATE managed_inventory_authoring_heads SET revision_id=$3 WHERE owner_id=$1 AND entry_id=$2 AND (revision_id=$4 OR revision_id=$3)",owner as _,entry as _,revision as _,expected as _).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(invalid(
                "Authoring observation changed concurrently; retry baseline preparation",
            ));
        }
        Ok(())
    }
}

impl ManagedRepository {
    pub async fn inventory_reconciliations(
        &self,
        owner: &UserId,
        entry: &InventoryEntryId,
        after: Option<&ManagedReconciliationId>,
    ) -> Result<Vec<crate::inventory::InventoryReconciliation>> {
        let resource = self.inventory_entry(owner, entry).await?.resource_id;
        let rows=sqlx::query!(r#"SELECT id,status AS "status: ReconciliationStatus",upstream_base_revision_id,managed_candidate_revision_id,incoming_revision_id,resolved_revision_id FROM managed_reconciliations WHERE owner_id=$1 AND resource_id=$2 AND ($3::text IS NULL OR id>$3) ORDER BY id LIMIT 50"#,owner as _,resource as _,after as _).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| crate::inventory::InventoryReconciliation {
                id: ManagedReconciliationId::new(row.id),
                status: row.status,
                upstream_base_revision_id: ResourceRevisionId::new(row.upstream_base_revision_id),
                managed_candidate_revision_id: ResourceRevisionId::new(
                    row.managed_candidate_revision_id,
                ),
                incoming_revision_id: ResourceRevisionId::new(row.incoming_revision_id),
                resolved_revision_id: row.resolved_revision_id.map(ResourceRevisionId::new),
            })
            .collect())
    }
}
