//! Notification identifiers.
//!
//! Notifications, subscriptions and rules are UUID-backed.
//! [`NotificationStreamId`] is a declared stream key from
//! `services/notifications/*.yaml`, checked to 1-64 characters of
//! lowercase ASCII letters, digits, `_` and `-`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(NotificationId, uuid);
crate::define_id!(NotificationSubscriptionId, uuid);
crate::define_id!(NotificationRuleId, uuid);
crate::define_id!(NotificationStreamId, checked, validate_stream_id);

const STREAM_ID_MAX_LEN: usize = 64;

fn validate_stream_id(value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() {
        return Err(IdValidationError::empty("NotificationStreamId"));
    }
    if value.len() > STREAM_ID_MAX_LEN {
        return Err(IdValidationError::invalid(
            "NotificationStreamId",
            "must be at most 64 characters",
        ));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err(IdValidationError::invalid(
            "NotificationStreamId",
            "must contain only lowercase letters, digits, '_' and '-'",
        ));
    }
    Ok(())
}
