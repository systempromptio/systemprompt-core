//! `Has*` capability traits and the [`CapabilityContext`] composition
//! helper.
//!
//! Extension authors mix these traits into their context type to advertise
//! which subsystems they need access to. The host application can then
//! satisfy each capability independently — for example, an extension that
//! requires only configuration access does not need the host to wire up a
//! database handle.

use std::sync::Arc;

use systemprompt_traits::{ConfigProvider, DatabaseHandle};

use crate::types::ExtensionType;

/// Marker capability: the implementor exposes a configuration provider.
pub trait HasConfig: Send + Sync {
    /// Concrete configuration provider type.
    type Config: ConfigProvider;

    /// Returns the configuration provider.
    fn config(&self) -> &Self::Config;
}

/// Marker capability: the implementor exposes a database handle.
pub trait HasDatabase: Send + Sync {
    /// Concrete database-handle type.
    type Database: DatabaseHandle;

    /// Returns the database handle.
    fn database(&self) -> &Self::Database;
}

/// Marker capability: the implementor exposes a sibling extension `E`.
pub trait HasExtension<E: ExtensionType>: Send + Sync {
    /// Returns the sibling extension instance.
    fn extension(&self) -> &E;
}

/// Marker capability: the implementor exposes a shared HTTP client.
pub trait HasHttpClient: Send + Sync {
    /// Returns the shared `reqwest::Client`.
    fn http_client(&self) -> &reqwest::Client;
}

/// Marker capability: the implementor exposes an event-bus publisher.
pub trait HasEventBus: Send + Sync {
    /// Concrete publisher type.
    type Publisher: systemprompt_traits::UserEventPublisher + Send + Sync;

    /// Returns the publisher.
    fn event_bus(&self) -> &Self::Publisher;
}

/// Marker capability: the implementor exposes an analytics sink.
pub trait HasAnalytics: Send + Sync {
    /// Concrete analytics sink type.
    type Analytics: Send + Sync;

    /// Returns the analytics sink.
    fn analytics(&self) -> &Self::Analytics;
}

/// Marker capability: the implementor exposes a request-fingerprint
/// resolver.
pub trait HasFingerprint: Send + Sync {
    /// Concrete fingerprint type.
    type Fingerprint: Send + Sync;

    /// Returns the fingerprint, if one is available for the current
    /// request.
    fn fingerprint(&self) -> Option<&Self::Fingerprint>;
}

/// Marker capability: the implementor exposes a user-service handle.
pub trait HasUserService: Send + Sync {
    /// Concrete user-service type.
    type UserService: Send + Sync;

    /// Returns the user service, if one is wired in.
    fn user_service(&self) -> Option<&Self::UserService>;
}

/// Marker capability: the implementor exposes a route classifier.
pub trait HasRouteClassifier: Send + Sync {
    /// Concrete classifier type.
    type RouteClassifier: Send + Sync;

    /// Returns the classifier.
    fn route_classifier(&self) -> &Self::RouteClassifier;
}

/// Convenience trait satisfied by any context that provides config,
/// database, and event-bus access.
pub trait FullContext: HasConfig + HasDatabase + HasEventBus {}

impl<T: HasConfig + HasDatabase + HasEventBus> FullContext for T {}

/// Generic context holding config, database, and event-bus capabilities
/// behind `Arc`s. Useful for assembling a [`FullContext`] without writing
/// a bespoke struct per binary.
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
    /// Constructs a [`CapabilityContext`] from the three Arc-wrapped
    /// capabilities.
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
