//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_identifiers::{DeviceId, UserId};

#[derive(Serialize, schemars::JsonSchema)]
pub struct IssuedConsumerCredential {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

impl std::fmt::Debug for IssuedConsumerCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedConsumerCredential")
            .field("device_id", &self.device_id)
            .field("consumer_id", &self.consumer_id)
            .finish_non_exhaustive()
    }
}
