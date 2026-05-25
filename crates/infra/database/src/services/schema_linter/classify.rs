//! Per-AST-node classification: which top-level statements are declarative-safe
//! and which are rejected as imperative.

use pg_query::protobuf::node::Node;
use pg_query::protobuf::{CreateStmt, DropStmt, ObjectType};

use super::location::StmtLoc;
use super::{LintError, LintSeverity};

pub(super) fn warn_create_table_missing_if_not_exists(
    create: &CreateStmt,
    loc: &StmtLoc<'_>,
) -> Option<LintError> {
    if create.if_not_exists {
        return None;
    }
    Some(LintError {
        line: loc.line,
        column: loc.col,
        severity: LintSeverity::Warning,
        message: "CREATE TABLE without IF NOT EXISTS — add IF NOT EXISTS for idempotency".into(),
        source: loc.source.to_owned(),
    })
}

pub(super) fn imperative_reason(node: &Node) -> Option<&'static str> {
    Some(match node {
        Node::AlterTableStmt(_) => "ALTER TABLE",
        Node::DropStmt(drop) if is_safe_drop(drop) => return None,
        Node::DropStmt(_) => "DROP",
        Node::InsertStmt(_) => "INSERT",
        Node::UpdateStmt(_) => "UPDATE",
        Node::DeleteStmt(_) => "DELETE",
        Node::TruncateStmt(_) => "TRUNCATE",
        Node::GrantStmt(_) => "GRANT/REVOKE",
        Node::RenameStmt(_) => "RENAME",
        Node::DoStmt(_) => "DO $$ block",
        Node::SelectStmt(_) => "SELECT",
        Node::CopyStmt(_) => "COPY",
        Node::AlterDatabaseStmt(_)
        | Node::AlterDatabaseSetStmt(_)
        | Node::AlterRoleStmt(_)
        | Node::AlterRoleSetStmt(_)
        | Node::AlterOwnerStmt(_)
        | Node::AlterSeqStmt(_)
        | Node::AlterEnumStmt(_)
        | Node::AlterFunctionStmt(_)
        | Node::AlterObjectSchemaStmt(_)
        | Node::AlterDefaultPrivilegesStmt(_) => "ALTER",
        _ => return None,
    })
}

/// `DROP` of a stateless derived object — `VIEW`, `MATERIALIZED VIEW`,
/// `INDEX`, or `TRIGGER` — guarded by `IF EXISTS` is declarative-safe: the
/// object holds no data and is rebuilt from the same schema file on the next
/// statement. `DROP TABLE` / `DROP COLUMN` are not — they destroy data and
/// must move to a migration.
fn is_safe_drop(drop: &DropStmt) -> bool {
    if !drop.missing_ok {
        return false;
    }
    matches!(
        ObjectType::try_from(drop.remove_type),
        Ok(ObjectType::ObjectView
            | ObjectType::ObjectMatview
            | ObjectType::ObjectIndex
            | ObjectType::ObjectTrigger)
    )
}
