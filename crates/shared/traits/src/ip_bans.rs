//! The IP ban list as seen by the detectors that feed it.
//!
//! The ban table belongs to the users crate; the detectors that decide an IP
//! should be banned (behavioural analysis, the malicious-IP blacklist) belong
//! to analytics. They meet through [`IpBanStore`], which the users crate
//! implements and the job context carries as [`DynIpBanStore`]. A job that
//! finds no store in its context fails rather than run without enforcement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
#[error("IP ban store: {0}")]
pub struct IpBanStoreError(#[source] pub Box<dyn StdError + Send + Sync>);

/// A ban a detector asks for, lasting `duration_days`.
#[derive(Debug, Clone, Copy)]
pub struct IpBanRequest<'a> {
    pub ip_address: &'a str,
    pub reason: &'a str,
    pub duration_days: i64,
    pub source_fingerprint: Option<&'a str>,
    pub ban_source: &'a str,
}

/// Read and write access to the IP ban list. `#[async_trait]` keeps the trait
/// `dyn`-compatible for [`DynIpBanStore`].
#[async_trait]
pub trait IpBanStore: Send + Sync {
    async fn is_banned(&self, ip_address: &str) -> Result<bool, IpBanStoreError>;

    async fn ban_ip(&self, request: IpBanRequest<'_>) -> Result<(), IpBanStoreError>;

    async fn cleanup_expired(&self) -> Result<u64, IpBanStoreError>;
}

pub type DynIpBanStore = Arc<dyn IpBanStore>;
