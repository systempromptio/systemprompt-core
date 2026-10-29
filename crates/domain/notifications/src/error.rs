//! [`NotificationsError`]: every failure the notifications service, pipeline
//! and consumer report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_events::RelayError;
use systemprompt_extension::notifications::{
    ChannelError, NotificationAudienceError, NotificationRouterError, NotificationSinkError,
    PublishPolicyError, VisibilityError,
};
use systemprompt_identifiers::{NotificationId, NotificationStreamId, NotificationSubscriptionId};
use systemprompt_models::notifications::{Attention, DeclError, NotificationKindError};
use systemprompt_traits::RepositoryError;

pub type NotificationsResult<T> = Result<T, NotificationsError>;

/// Why a notifications operation failed.
#[derive(Debug, thiserror::Error)]
pub enum NotificationsError {
    #[error("notification storage failed")]
    Repository(#[from] RepositoryError),
    #[error("notification database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("notification event could not be emitted")]
    Sink(#[from] NotificationSinkError),
    #[error("notification routing failed")]
    Router(#[from] NotificationRouterError),
    #[error("notification audience could not be resolved")]
    Audience(#[from] NotificationAudienceError),
    #[error("notification visibility could not be decided")]
    Visibility(#[from] VisibilityError),
    #[error("notification channel failed")]
    Channel(#[from] ChannelError),
    #[error("publish policy failed")]
    PublishPolicy(#[from] PublishPolicyError),
    #[error("notification {id} not found")]
    NotFound { id: NotificationId },
    #[error("notification subscription {id} not found")]
    SubscriptionNotFound { id: NotificationSubscriptionId },
    #[error("notification stream {stream} not found")]
    StreamNotFound { stream: NotificationStreamId },
    #[error("caller is not in the audience of stream {stream}")]
    NotInAudience { stream: NotificationStreamId },
    #[error("{entity} {key} is declared by configuration")]
    DeclaredByConfig { entity: &'static str, key: String },
    #[error("publish refused: {reason}")]
    PublishRefused { reason: String },
    #[error("notification {id} at {attention} attention cannot be escalated")]
    NotEscalatable {
        id: NotificationId,
        attention: Attention,
    },
    #[error("invalid notification declaration")]
    InvalidDeclaration(#[from] DeclError),
    #[error("invalid notification kind")]
    Kind(#[from] NotificationKindError),
    #[error("notification payload could not be encoded")]
    Encode(#[from] serde_json::Error),
    #[error("realtime event {event} could not be relayed")]
    Events {
        event: &'static str,
        #[source]
        source: RelayError,
    },
}
