//! Service layer: provider abstraction, owned [`Database`] handle,
//! transaction helpers, batch execution, and CLI display traits.

pub mod database;
pub mod display;
pub mod executor;
pub mod postgres;
pub mod provider;
pub mod schema_additivity;
pub mod transaction;

pub use database::{Database, DatabaseExt, DbPool};
pub use display::DatabaseCliDisplay;
pub use executor::SqlExecutor;
pub use postgres::PostgresProvider;
pub use postgres::transaction::PostgresTransaction;
pub use provider::{DatabaseProvider, DatabaseProviderExt};
pub use transaction::{BoxFuture, with_transaction, with_transaction_raw, with_transaction_retry};
