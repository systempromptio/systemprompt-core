//! Tables a user's rows must leave with them.
//!
//! Most activity tables key on `user_id` without a foreign key — the column
//! also carries sentinel principals such as `system`, and a session id means
//! three different things across the schema — so the database cannot cascade
//! a user delete. These declarations are the substitute: every member that
//! owns a user-keyed table declares it in `contribute`, and
//! `UserRepository::delete` runs every member's set inside one transaction,
//! in composition order, before the `users` row goes:
//!
//! ```ignore
//! c.user_purge(UserPurgeTable { table: "plugin_usage_events", column: "user_id", predicate: None })
//!     .user_purge(UserPurgeTable {
//!         table: "document_grants",
//!         column: "subject_value",
//!         predicate: Some(PurgePredicate { column: "subject_kind", any_of: &["user"] }),
//!     });
//! ```
//!
//! A [`PurgePredicate`] narrows a table whose rows for that user are not all
//! the user's to purge, to the rows whose column holds one of a fixed set of
//! values. It is not SQL: the purge renders the column as a quoted identifier
//! and binds the values, so no declaration can widen the delete beyond the
//! user's own rows.
//!
//! Content shared between users — a body several artifacts point at — is not
//! keyed by user at all. Such a table is declared as an orphan sweep: after
//! the user-keyed tables are cleared, every row of it that nothing references
//! any more is deleted in the same transaction, and the dry run counts the
//! rows only this user's references keep alive:
//!
//! ```ignore
//! c.orphan_sweep(OrphanSweep {
//!     table: "artifact_payloads",
//!     key: "sha256",
//!     referenced_by: "mcp_artifacts",
//!     via: "payload_sha256",
//!     user_column: "user_id",
//! });
//! ```
//!
//! Both are admitted only over the declaring member's own tables
//! ([`AdmittedPurge`](crate::AdmittedPurge)). The installer records each
//! member's declarations as [`PurgeDeclaration`]s, so a purge also covers a
//! member the profile has disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// One table to clear by `column = <user id>` when a user is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserPurgeTable {
    pub table: &'static str,
    pub column: &'static str,
    pub predicate: Option<PurgePredicate>,
}

/// Narrows a purge table to the rows whose `column` holds one of `any_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgePredicate {
    pub column: &'static str,
    pub any_of: &'static [&'static str],
}

/// One table whose rows exist only while `referenced_by.via` points at
/// `table.key`.
///
/// `user_column` is the user key of `referenced_by`. A member
/// declares it over `&'static str` names; the recorded form in
/// [`PurgeDeclaration::Sweep`] is the same type over owned names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrphanSweep<S = &'static str> {
    pub table: S,
    pub key: S,
    pub referenced_by: S,
    pub via: S,
    pub user_column: S,
}

impl<S: AsRef<str>> OrphanSweep<S> {
    #[must_use]
    pub fn names(&self) -> [&str; 5] {
        [
            self.table.as_ref(),
            self.key.as_ref(),
            self.referenced_by.as_ref(),
            self.via.as_ref(),
            self.user_column.as_ref(),
        ]
    }

    #[must_use]
    pub fn to_owned_names(&self) -> OrphanSweep<String> {
        let [table, key, referenced_by, via, user_column] = self.names().map(str::to_owned);
        OrphanSweep {
            table,
            key,
            referenced_by,
            via,
            user_column,
        }
    }
}

/// A purge table or orphan sweep in owned form: what the installer records
/// and what the user purge runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PurgeDeclaration {
    Table {
        table: String,
        column: String,
        predicate: Option<PurgeFilter>,
    },
    Sweep(OrphanSweep<String>),
}

/// The owned form of a [`PurgePredicate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeFilter {
    pub column: String,
    pub any_of: Vec<String>,
}

impl PurgeDeclaration {
    #[must_use]
    pub fn table(&self) -> &str {
        match self {
            Self::Table { table, .. } | Self::Sweep(OrphanSweep { table, .. }) => table,
        }
    }

    #[must_use]
    pub fn tables(&self) -> Vec<&str> {
        match self {
            Self::Table { table, .. } => vec![table],
            Self::Sweep(sweep) => vec![&sweep.table, &sweep.referenced_by],
        }
    }

    #[must_use]
    pub fn identifiers(&self) -> Vec<&str> {
        match self {
            Self::Table {
                table,
                column,
                predicate,
            } => {
                let mut names = vec![table.as_str(), column.as_str()];
                names.extend(predicate.iter().map(|p| p.column.as_str()));
                names
            },
            Self::Sweep(sweep) => sweep.names().to_vec(),
        }
    }

    #[must_use]
    pub const fn is_sweep(&self) -> bool {
        matches!(self, Self::Sweep(_))
    }
}

impl From<UserPurgeTable> for PurgeDeclaration {
    fn from(entry: UserPurgeTable) -> Self {
        Self::Table {
            table: entry.table.to_owned(),
            column: entry.column.to_owned(),
            predicate: entry.predicate.map(|p| PurgeFilter {
                column: p.column.to_owned(),
                any_of: p.any_of.iter().map(|v| (*v).to_owned()).collect(),
            }),
        }
    }
}

impl From<OrphanSweep> for PurgeDeclaration {
    fn from(sweep: OrphanSweep) -> Self {
        Self::Sweep(sweep.to_owned_names())
    }
}
