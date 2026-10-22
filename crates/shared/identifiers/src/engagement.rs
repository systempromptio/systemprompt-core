//! Page-engagement identifiers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(EngagementEventId, generate);
crate::define_id!(AnalyticsEventId, checked, |value| {
    crate::macros::validate_non_empty("AnalyticsEventId", value)
});
