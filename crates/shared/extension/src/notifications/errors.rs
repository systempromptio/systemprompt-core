//! Error types for the notification seams.

use systemprompt_models::notifications::AudienceSubject;
use systemprompt_traits::{BoxedSource, RepositoryError};

/// Why emitting a notification event failed.
#[derive(Debug, thiserror::Error)]
pub enum NotificationSinkError {
    #[error("notification event could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[cfg(feature = "sqlx")]
    #[error("notification outbox write failed")]
    Database(#[from] sqlx::Error),
    #[error("notification outbox write failed")]
    Repository(#[from] RepositoryError),
    #[error("notifications are not built into this server")]
    Unavailable,
}

/// Why a router could not plan an event.
#[derive(Debug, thiserror::Error)]
pub enum NotificationRouterError {
    #[cfg(feature = "sqlx")]
    #[error("notification routing query failed")]
    Database(#[from] sqlx::Error),
    #[error("notification routing query failed")]
    Repository(#[from] RepositoryError),
    #[error("notification router {router} failed")]
    Other {
        router: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why an audience could not be expanded.
#[derive(Debug, thiserror::Error)]
pub enum NotificationAudienceError {
    #[cfg(feature = "sqlx")]
    #[error("audience query failed")]
    Database(#[from] sqlx::Error),
    #[error("audience query failed")]
    Repository(#[from] RepositoryError),
    #[error("audience {subject} has more than {limit} members")]
    TooLarge {
        subject: AudienceSubject,
        limit: usize,
    },
    #[error("audience resolver {resolver} failed")]
    Other {
        resolver: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why a publish policy could not reach a verdict.
#[derive(Debug, thiserror::Error)]
pub enum PublishPolicyError {
    #[cfg(feature = "sqlx")]
    #[error("publish policy query failed")]
    Database(#[from] sqlx::Error),
    #[error("publish policy query failed")]
    Repository(#[from] RepositoryError),
    #[error("publish policy {policy} failed")]
    Other {
        policy: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Why visibility of a resource could not be decided.
#[derive(Debug, thiserror::Error)]
pub enum VisibilityError {
    #[cfg(feature = "sqlx")]
    #[error("visibility query failed")]
    Database(#[from] sqlx::Error),
    #[error("visibility query failed")]
    Repository(#[from] RepositoryError),
    #[error("visibility check for {kind} failed")]
    Other {
        kind: String,
        #[source]
        source: BoxedSource,
    },
}

/// Why a channel could not deliver a notification.
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("notification payload could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[error("channel {surface} is unavailable")]
    Unavailable { surface: &'static str },
    #[error("channel {surface} failed")]
    Other {
        surface: &'static str,
        #[source]
        source: BoxedSource,
    },
}
