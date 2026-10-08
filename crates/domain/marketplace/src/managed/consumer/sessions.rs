//! Binding a verified installation receipt to a native host session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::InstallationSessionBindingId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerSessionBinding {
    pub id: InstallationSessionBindingId,
    pub bound_at: DateTime<Utc>,
}
