//! Tables a user's rows must leave with them.
//!
//! Most activity tables key on `user_id` without a foreign key — the column
//! also carries sentinel principals such as `system`, and a session id means
//! three different things across the schema — so the database cannot cascade
//! a user delete. This registry is the substitute: every crate that owns a
//! user-keyed table declares it once, and `UserRepository::delete` runs the
//! whole set inside the privacy transaction, in registration order, before
//! the `users` row goes. An extension declares its own tables with
//! `user_purge_tables!`; nothing else needs to know the list exists:
//!
//! ```ignore
//! user_purge_tables!("my-extension", [
//!     ("plugin_usage_events", "user_id"),
//!     ("session_ratings", "user_id"),
//! ]);
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One table to clear by `column = <user id>` when a user is deleted.
#[derive(Debug, Clone, Copy)]
pub struct UserPurgeTable {
    pub table: &'static str,
    pub column: &'static str,
    pub owner: &'static str,
}

inventory::collect!(UserPurgeTable);

pub fn registered_user_purge_tables() -> impl Iterator<Item = &'static UserPurgeTable> {
    inventory::iter::<UserPurgeTable>.into_iter()
}

#[macro_export]
macro_rules! user_purge_tables {
    ($owner:literal, [ $( ($table:literal, $column:literal) ),* $(,)? ]) => {
        $(
            ::inventory::submit! {
                $crate::purge::UserPurgeTable {
                    table: $table,
                    column: $column,
                    owner: $owner,
                }
            }
        )*
    };
}
