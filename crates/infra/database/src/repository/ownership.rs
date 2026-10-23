//! One implementation of [`OwnerReassignment`] for every domain that owns
//! user-keyed rows.
//!
//! [`owner_reassignment!`](crate::owner_reassignment) declares a repository
//! struct and its [`OwnerReassignment`] impl from a list of per-table
//! statements. Each statement stays a compile-time `sqlx::query!` expanded
//! in the declaring crate, so its offline cache entry lives with the crate
//! that owns the table. The generated impl runs every statement in one
//! transaction and reports the rows each one touched, in declaration order.
//!
//! An `update` statement binds the source user as `$1` and the target user
//! as `$2`; a `delete` statement binds only the source user as `$1`, for
//! rows that are credentials minted for the old user and must not be rebound.
//!
//! ```ignore
//! systemprompt_database::owner_reassignment! {
//!     pub struct FilesOwnerReassignment;
//!     domain = "files";
//!     update "files" => "UPDATE files SET user_id = $2 WHERE user_id = $1";
//! }
//! ```
//!
//! [`OwnerReassignment`]: systemprompt_traits::OwnerReassignment
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[macro_export]
macro_rules! owner_reassignment {
    (
        $vis:vis struct $name:ident;
        domain = $domain:literal;
        $( $kind:ident $table:literal => $sql:literal; )+
    ) => {
        #[derive(Debug, Clone)]
        $vis struct $name {
            write_pool: ::std::sync::Arc<::sqlx::PgPool>,
        }

        impl $name {
            pub fn new(db: &$crate::DbPool) -> Self {
                Self {
                    write_pool: db.write_pool(),
                }
            }
        }

        #[::async_trait::async_trait]
        impl ::systemprompt_traits::OwnerReassignment for $name {
            fn domain(&self) -> &'static str {
                $domain
            }

            async fn reassign_owner(
                &self,
                from: &::systemprompt_identifiers::UserId,
                to: &::systemprompt_identifiers::UserId,
            ) -> ::std::result::Result<
                ::systemprompt_traits::ReassignedRows,
                ::systemprompt_traits::RepositoryError,
            > {
                let mut tx = self.write_pool.begin().await?;
                let mut tables = ::std::vec::Vec::new();
                $(
                    let rows = $crate::owner_reassignment!(@step $kind tx from to $sql);
                    tables.push(($table, rows));
                )+
                tx.commit().await?;
                Ok(::systemprompt_traits::ReassignedRows { tables })
            }
        }
    };
    (@step update $tx:ident $from:ident $to:ident $sql:literal) => {
        ::sqlx::query!($sql, $from.as_str(), $to.as_str())
            .execute(&mut *$tx)
            .await?
            .rows_affected()
    };
    (@step delete $tx:ident $from:ident $to:ident $sql:literal) => {
        ::sqlx::query!($sql, $from.as_str())
            .execute(&mut *$tx)
            .await?
            .rows_affected()
    };
}
