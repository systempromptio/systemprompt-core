//! Answers of the device-authenticated consumer API that
//! [`crate::feedback`] does not already name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::InstallationSessionBindingId;

/// Answer of `POST /api/v1/consumer/session-bindings`; the same binding again
/// answers the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBinding {
    pub id: InstallationSessionBindingId,
    pub bound_at: DateTime<Utc>,
}
