//! [`Withheld`]: why one recipient, or one audience subject, received
//! nothing. Its `Display` is the stable reason code a
//! `notification_delivery_failures` row carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::notifications::{NotificationAudienceError, VisibilityError};

/// Why delivery to a recipient or an audience subject was withheld.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Withheld {
    #[error("not_in_audience")]
    NotInAudience,
    #[error("audience_unknown")]
    AudienceUnknown,
    #[error("audience_over_limit")]
    OverLimit { limit: usize },
    #[error("audience_timeout")]
    Timeout,
    #[error("no_resolver")]
    NoResolver,
    #[error("hidden")]
    Hidden { kind: String },
    #[error("no_visibility_provider")]
    NoProvider { kind: String },
    #[error("audience_resolver_failed")]
    Resolver(#[source] Arc<NotificationAudienceError>),
    #[error("visibility_failed")]
    Visibility(#[source] Arc<VisibilityError>),
}

impl Withheld {
    #[must_use]
    pub const fn decision(&self) -> &'static str {
        match self {
            Self::NotInAudience => "not_in_audience",
            Self::AudienceUnknown => "audience_unknown",
            Self::OverLimit { .. } => "audience_over_limit",
            Self::Timeout => "audience_timeout",
            Self::NoResolver => "no_resolver",
            Self::Hidden { .. } => "hidden",
            Self::NoProvider { .. } => "no_visibility_provider",
            Self::Resolver(_) => "audience_resolver_failed",
            Self::Visibility(_) => "visibility_failed",
        }
    }
}
