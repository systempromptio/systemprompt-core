//! The MCP member's user-data contract: the per-user rows a user purge
//! deletes, the artifact payloads an orphan sweep collects once no artifact
//! references them, and how each table's rows move when an owner is
//! reassigned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub(crate) fn contribute(c: &mut Contributions<'_>) {
    c.user_purge(UserPurgeTable {
        table: "mcp_artifacts",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_tool_executions",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_sessions",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_external_sessions",
        column: "user_id",
        predicate: None,
    });
    c.orphan_sweep(OrphanSweep {
        table: "artifact_payloads",
        key: "sha256",
        referenced_by: "mcp_artifacts",
        via: "payload_sha256",
        user_column: "user_id",
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_tool_executions",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_artifacts",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_sessions",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_external_sessions",
        column: "user_id",
        action: ReassignAction::DELETE,
    });
}
