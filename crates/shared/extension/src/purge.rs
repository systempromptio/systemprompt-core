//! Tables a user's rows must leave with them.
//!
//! Most activity tables key on `user_id` without a foreign key — the column
//! also carries sentinel principals such as `system`, and a session id means
//! three different things across the schema — so the database cannot cascade
//! a user delete. This registry is the substitute: every crate that owns a
//! user-keyed table declares it once, and `UserRepository::delete` runs the
//! whole set inside one transaction, in registration order, before
//! the `users` row goes. An extension declares its own tables with
//! `user_purge_tables!`; nothing else needs to know the list exists:
//!
//! ```ignore
//! user_purge_tables!("my-extension", [
//!     ("plugin_usage_events", "user_id"),
//!     ("session_ratings", "user_id", "channel <> 'reporting'"),
//! ]);
//! ```
//!
//! The optional third element is a fixed SQL predicate conjoined with the user
//! match, for a table whose rows for that user are not all the user's to
//! purge.
//!
//! Content shared between users — a body several artifacts point at — is not
//! keyed by user at all. Such a table is declared as an orphan sweep: after
//! the user-keyed tables are cleared, every row of it that nothing references
//! any more is deleted in the same transaction, and the dry run counts the
//! rows only this user's references keep alive:
//!
//! ```ignore
//! orphan_sweeps!("my-extension", [{
//!     table: "artifact_payloads",
//!     key: "sha256",
//!     referenced_by: "mcp_artifacts",
//!     via: "payload_sha256",
//!     user_column: "user_id",
//! }]);
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One table to clear by `column = <user id>` when a user is deleted.
#[derive(Debug, Clone, Copy)]
pub struct UserPurgeTable {
    pub table: &'static str,
    pub column: &'static str,
    pub predicate: Option<&'static str>,
    pub owner: &'static str,
}

inventory::collect!(UserPurgeTable);

pub fn registered_user_purge_tables() -> impl Iterator<Item = &'static UserPurgeTable> {
    inventory::iter::<UserPurgeTable>.into_iter()
}

/// One table whose rows exist only while `referenced_by.via` points at
/// `table.key`; `user_column` is the user key of `referenced_by`.
#[derive(Debug, Clone, Copy)]
pub struct OrphanSweep {
    pub table: &'static str,
    pub key: &'static str,
    pub referenced_by: &'static str,
    pub via: &'static str,
    pub user_column: &'static str,
    pub owner: &'static str,
}

inventory::collect!(OrphanSweep);

pub fn registered_orphan_sweeps() -> impl Iterator<Item = &'static OrphanSweep> {
    inventory::iter::<OrphanSweep>.into_iter()
}

#[doc(hidden)]
#[macro_export]
macro_rules! __user_purge_predicate {
    () => {
        ::core::option::Option::None
    };
    ($predicate:literal) => {
        ::core::option::Option::Some($predicate)
    };
}

#[macro_export]
macro_rules! user_purge_tables {
    ($owner:literal, [ $( ($table:literal, $column:literal $(, $predicate:literal)?) ),* $(,)? ]) => {
        $(
            ::inventory::submit! {
                $crate::purge::UserPurgeTable {
                    table: $table,
                    column: $column,
                    predicate: $crate::__user_purge_predicate!($($predicate)?),
                    owner: $owner,
                }
            }
        )*
    };
}

#[macro_export]
macro_rules! orphan_sweeps {
    ($owner:literal, [ $( { $($field:ident : $value:literal),* $(,)? } ),* $(,)? ]) => {
        $(
            ::inventory::submit! {
                $crate::purge::OrphanSweep {
                    $($field: $value,)*
                    owner: $owner,
                }
            }
        )*
    };
}
