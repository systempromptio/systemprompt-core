use std::sync::Arc;

use systemprompt_traits::{ConfigProvider, DatabaseHandle};

use crate::types::ExtensionType;

pub trait HasConfig: Send + Sync {
    type Config: ConfigProvider;

    fn config(&self) -> &Self::Config;
}

pub trait HasDatabase: Send + Sync {
    type Database: DatabaseHandle;

    fn database(&self) -> &Self::Database;
}

pub trait HasExtension<E: ExtensionType>: Send + Sync {
    fn extension(&self) -> &E;
}

#[cfg(feature = "axum")]
pub trait HasHttpClient: Send + Sync {
    fn http_client(&self) -> &reqwest::Client;
}

pub trait HasEventBus: Send + Sync {
    type Publisher: systemprompt_traits::UserEventPublisher + Send + Sync;

    fn event_bus(&self) -> &Self::Publisher;
}

pub trait FullContext: HasConfig + HasDatabase + HasEventBus {}

impl<T: HasConfig + HasDatabase + HasEventBus> FullContext for T {}

#[derive(Debug)]
pub struct CapabilityContext<C, D, E> {
    config: Arc<C>,
    database: Arc<D>,
    event_bus: Arc<E>,
}

impl<C, D, E> CapabilityContext<C, D, E>
where
    C: ConfigProvider,
    D: DatabaseHandle,
    E: systemprompt_traits::UserEventPublisher + Send + Sync,
{
    #[must_use]
    pub const fn new(config: Arc<C>, database: Arc<D>, event_bus: Arc<E>) -> Self {
        Self {
            config,
            database,
            event_bus,
        }
    }
}

impl<C, D, E> HasConfig for CapabilityContext<C, D, E>
where
    C: ConfigProvider + Send + Sync,
    D: Send + Sync,
    E: Send + Sync,
{
    type Config = C;

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl<C, D, E> HasDatabase for CapabilityContext<C, D, E>
where
    C: Send + Sync,
    D: DatabaseHandle + Send + Sync,
    E: Send + Sync,
{
    type Database = D;

    fn database(&self) -> &Self::Database {
        &self.database
    }
}

impl<C, D, E> HasEventBus for CapabilityContext<C, D, E>
where
    C: Send + Sync,
    D: Send + Sync,
    E: systemprompt_traits::UserEventPublisher + Send + Sync,
{
    type Publisher = E;

    fn event_bus(&self) -> &Self::Publisher {
        &self.event_bus
    }
}
