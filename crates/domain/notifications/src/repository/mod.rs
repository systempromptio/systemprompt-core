//! Repositories over the notifications tables, built once into
//! [`NotificationRepositories`] and shared by the service, pipeline and jobs.
//!
//! A query the consumer runs inside its claimed outbox transaction, or the
//! config plane inside the host's apply transaction, takes the executor as an
//! argument; everything else runs on the repository's own pools.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod failures;
mod notifications;
mod persist;
mod receipts;
mod retention;
mod rules;
mod stage;
mod state;
mod streams;
mod subscription_writes;
mod subscriptions;
mod sweep;
mod unread;

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

pub use failures::{DeliveryFailure, DeliveryHistory, NewFailure, NotificationFailureRepository};
pub use notifications::NotificationRepository;
pub use receipts::NotificationReceiptRepository;
pub use rules::NotificationRuleRepository;
pub use stage::{FailureStage, FailureStageError};
pub use streams::NotificationStreamRepository;
pub use subscriptions::NotificationSubscriptionRepository;
pub use sweep::SWEEP_LOCK_KEY;
pub use unread::UNREAD_CAP;

/// Every notifications repository, constructed together from one pool.
#[derive(Debug, Clone)]
pub struct NotificationRepositories {
    pub streams: Arc<NotificationStreamRepository>,
    pub subscriptions: Arc<NotificationSubscriptionRepository>,
    pub rules: Arc<NotificationRuleRepository>,
    pub notifications: Arc<NotificationRepository>,
    pub receipts: Arc<NotificationReceiptRepository>,
    pub failures: Arc<NotificationFailureRepository>,
    pools: Pools,
}

impl NotificationRepositories {
    pub fn new(db: &DbPool) -> Self {
        let pools = Pools::new(db);
        Self {
            streams: Arc::new(NotificationStreamRepository::new(pools.clone())),
            subscriptions: Arc::new(NotificationSubscriptionRepository::new(pools.clone())),
            rules: Arc::new(NotificationRuleRepository::new(pools.clone())),
            notifications: Arc::new(NotificationRepository::new(pools.clone())),
            receipts: Arc::new(NotificationReceiptRepository::new(pools.clone())),
            failures: Arc::new(NotificationFailureRepository::new(pools.clone())),
            pools,
        }
    }

    #[must_use]
    pub fn read_pool(&self) -> &PgPool {
        &self.pools.read
    }

    #[must_use]
    pub fn write_pool(&self) -> &PgPool {
        &self.pools.write
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Pools {
    pub(crate) read: Arc<PgPool>,
    pub(crate) write: Arc<PgPool>,
}

impl Pools {
    fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }
}
