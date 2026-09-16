//! Inventory and reconciliation contracts include bounded traversal and status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::evaluation::collections::Page;
use crate::routes::evaluation::inventory as api;
use crate::routes::evaluation::operations::OperationResponse;
use systemprompt_marketplace::inventory::{
    BaselineCapture, BaselinePreparation, InstallationCoverage, InstallationCoverageStatus,
    InventoryEntry, InventoryGitBinding, InventoryReconciliation, InventoryStatus,
    LatestPublication, ObservedMembership,
};
pub(super) fn register(d: &mut Document) {
    d.add::<(), api::Page>("/inventory", "get", 200, false);
    d.query::<api::Cursor>("/inventory", "get");
    d.add::<(), InventoryStatus>("/inventory/status", "get", 200, false);
    d.add::<(), InstallationCoverageStatus>("/inventory/installations/status", "get", 200, false);
    d.add::<(), Option<InstallationCoverage>>(
        "/inventory/{id}/installation-coverage",
        "get",
        200,
        false,
    );
    d.add::<(), OperationResponse<InventoryStatus>>(
        "/inventory/reconciliations",
        "post",
        200,
        false,
    );
    d.idempotent("/inventory/reconciliations");
    d.add::<BaselinePreparation, Page<BaselineCapture>>("/inventory/baselines", "post", 200, false);
    d.add::<(), Vec<LatestPublication>>("/inventory/publications/latest", "post", 200, false);
    d.add::<(), InventoryEntry>("/inventory/{id}", "get", 200, false);
    d.add::<api::Binding, ()>("/inventory/{id}/bindings", "post", 204, false);
    d.add::<(), ObservedMembership>("/inventory/{id}/membership", "get", 200, false);
    d.query::<api::MembershipTime>("/inventory/{id}/membership", "get");
    d.add::<(), Page<InventoryReconciliation>>(
        "/inventory/{id}/reconciliations",
        "get",
        200,
        false,
    );
    d.query::<api::ReconciliationCursor>("/inventory/{id}/reconciliations", "get");
    d.add::<(), Option<InventoryGitBinding>>("/inventory/{id}/git-binding", "get", 200, false);
    d.add::<(), BaselineCapture>("/inventory/{id}/captures/{operation}", "get", 200, false);
}
