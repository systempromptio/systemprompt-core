//! Generic log persistence trait.
//!
//! [`LogService`] carries associated types, so it is only ever dispatched
//! statically and uses native `async fn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use systemprompt_identifiers::LogId;

pub trait LogService: Send + Sync {
    type Entry: Send + Sync;
    type Filter: Send + Sync;
    type Error: std::error::Error + Send + Sync;

    fn log(&self, entry: Self::Entry) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn query(
        &self,
        filter: &Self::Filter,
    ) -> impl Future<Output = Result<(Vec<Self::Entry>, i64), Self::Error>> + Send;

    fn list_recent(
        &self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<Self::Entry>, Self::Error>> + Send;

    fn find_by_id(
        &self,
        id: &LogId,
    ) -> impl Future<Output = Result<Option<Self::Entry>, Self::Error>> + Send;

    fn delete(&self, id: &LogId) -> impl Future<Output = Result<bool, Self::Error>> + Send;
}
