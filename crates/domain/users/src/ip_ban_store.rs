//! [`BannedIpRepository`] as the shared [`IpBanStore`] the analytics
//! detectors ban through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_traits::{IpBanRequest, IpBanStore, IpBanStoreError};

use crate::repository::{BanDuration, BanIpParams, BannedIpRepository};

fn store_error(error: crate::UserError) -> IpBanStoreError {
    IpBanStoreError(Box::new(error))
}

#[async_trait]
impl IpBanStore for BannedIpRepository {
    async fn is_banned(&self, ip_address: &str) -> Result<bool, IpBanStoreError> {
        Self::is_banned(self, ip_address).await.map_err(store_error)
    }

    async fn ban_ip(&self, request: IpBanRequest<'_>) -> Result<(), IpBanStoreError> {
        let params = BanIpParams::new(
            request.ip_address,
            request.reason,
            BanDuration::Days(request.duration_days),
            request.ban_source,
        );
        let params = match request.source_fingerprint {
            Some(fingerprint) => params.with_source_fingerprint(fingerprint),
            None => params,
        };
        Self::ban_ip(self, params).await.map_err(store_error)
    }

    async fn cleanup_expired(&self) -> Result<u64, IpBanStoreError> {
        Self::cleanup_expired(self).await.map_err(store_error)
    }
}
