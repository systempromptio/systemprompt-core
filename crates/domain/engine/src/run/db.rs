//! The store, reached from async code: every call runs on the blocking pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use harness_store::Store;

use crate::error::EngineError;

#[derive(Debug, Clone)]
pub(crate) struct Db {
    store: Store,
}

impl Db {
    pub(crate) const fn new(store: Store) -> Self {
        Self { store }
    }

    pub(crate) async fn run<T, F>(&self, work: F) -> Result<T, EngineError>
    where
        T: Send + 'static,
        F: FnOnce(&Store) -> harness_store::Result<T> + Send + 'static,
    {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || work(&store))
            .await
            .map_err(EngineError::Task)?
            .map_err(EngineError::from)
    }
}
