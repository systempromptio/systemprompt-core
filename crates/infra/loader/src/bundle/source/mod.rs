//! Remote bundle transports.
//!
//! [`BundleFetcher`] is deliberately two calls: `head` is the cheap identity
//! probe the boot path uses to decide whether anything changed, and `fetch`
//! is the streaming download. `head` returns an empty digest when the remote
//! offers no cheap identity (an HTTPS endpoint without an `ETag`), which the
//! boot path reads as "unknown" and therefore as changed — never as
//! unchanged, which would pin an instance to a stale bundle forever.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod https;
pub mod oci;
mod stream;

use std::path::{Path, PathBuf};

use systemprompt_models::profile::ServicesSource;

use super::error::{BundleError, BundleResult};

pub use https::HttpsFetcher;
pub use oci::{OciFetcher, push_bundle};

pub const MAX_BUNDLE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRef {
    pub digest: String,
}

impl RemoteRef {
    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        self.digest.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct FetchedBundle {
    pub archive: PathBuf,
    pub digest: String,
}

pub trait BundleFetcher {
    fn head(&self) -> impl Future<Output = BundleResult<RemoteRef>> + Send;

    fn fetch(&self, into: &Path) -> impl Future<Output = BundleResult<FetchedBundle>> + Send;
}

#[derive(Debug)]
pub enum AnyFetcher {
    Https(HttpsFetcher),
    Oci(OciFetcher),
}

impl AnyFetcher {
    pub fn from_source(
        source: &ServicesSource,
        auth: Option<String>,
        client: &reqwest::Client,
    ) -> BundleResult<Self> {
        if !source.is_exactly_one() {
            return Err(BundleError::policy(format!(
                "source {} must declare exactly one of https: or oci:",
                source.name
            )));
        }
        if let Some(https) = source.https.as_ref() {
            return Ok(Self::Https(HttpsFetcher::new(
                &source.name,
                &https.url,
                auth,
                client.clone(),
            )));
        }
        let oci = source
            .oci
            .as_ref()
            .ok_or_else(|| BundleError::policy("source declares no transport"))?;
        Ok(Self::Oci(OciFetcher::new(
            &source.name,
            &oci.reference,
            auth,
            client.clone(),
        )?))
    }
}

impl BundleFetcher for AnyFetcher {
    async fn head(&self) -> BundleResult<RemoteRef> {
        match self {
            Self::Https(f) => f.head().await,
            Self::Oci(f) => f.head().await,
        }
    }

    async fn fetch(&self, into: &Path) -> BundleResult<FetchedBundle> {
        match self {
            Self::Https(f) => f.fetch(into).await,
            Self::Oci(f) => f.fetch(into).await,
        }
    }
}
