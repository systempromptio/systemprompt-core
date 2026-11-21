//! Columns an account merge moves from one user to another.
//!
//! Merging a user into another touches every member's user-keyed tables. A
//! member declares each column it owns as an [`OwnerReassignment`] in
//! `contribute`; it is data, not code. The kernel admits a declaration only
//! over a table the declaring member's own schema creates
//! ([`AdmittedOwnerReassignments`](crate::AdmittedOwnerReassignments)), and
//! the users domain renders every admitted declaration into one statement
//! from validated, quoted identifiers and runs them all, with the users-owned
//! half of the merge, in a single transaction. The table a declaration names
//! is the table its statement touches.
//!
//! ```ignore
//! c.owner_reassignment(OwnerReassignment {
//!     table: "agent_tasks",
//!     column: "user_id",
//!     action: ReassignAction::Update { touch: Some("updated_at"), keep_target_on: &[] },
//! })
//! .owner_reassignment(OwnerReassignment {
//!     table: "ai_quota_buckets",
//!     column: "subject_id",
//!     action: ReassignAction::Delete {
//!         predicate: Some(PurgePredicate { column: "subject_kind", any_of: &["user"] }),
//!     },
//! });
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::purge::PurgePredicate;

/// One user-keyed column an account merge moves from the source user to the
/// target user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerReassignment {
    pub table: &'static str,
    pub column: &'static str,
    pub action: ReassignAction,
}

/// What an account merge does to the source user's rows of one column.
///
/// - `Update` rewrites the column to the target; `touch` names a timestamp
///   column set to the current time, and `keep_target_on` names the columns
///   that, with the user column, identify a row: a source row whose counterpart
///   the target already holds is left in place rather than colliding with it.
/// - `Delete` removes the source user's rows (credentials or counters minted
///   for that user, which must not be rebound), optionally narrowed by a
///   [`PurgePredicate`].
/// - `ReplaceInArray` replaces the source id with the target id inside an array
///   column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReassignAction {
    Update {
        touch: Option<&'static str>,
        keep_target_on: &'static [&'static str],
    },
    Delete {
        predicate: Option<PurgePredicate>,
    },
    ReplaceInArray,
}

impl ReassignAction {
    pub const UPDATE: Self = Self::Update {
        touch: None,
        keep_target_on: &[],
    };

    pub const DELETE: Self = Self::Delete { predicate: None };
}

impl OwnerReassignment {
    #[must_use]
    pub fn identifiers(&self) -> Vec<&'static str> {
        let mut names = vec![self.table, self.column];
        match self.action {
            ReassignAction::Update {
                touch,
                keep_target_on,
            } => {
                names.extend(touch);
                names.extend_from_slice(keep_target_on);
            },
            ReassignAction::Delete { predicate } => {
                names.extend(predicate.map(|p| p.column));
            },
            ReassignAction::ReplaceInArray => {},
        }
        names
    }
}
