//! The narrow views the `gateway` member builds in its provide and confers on
//! `gateway-http` for the bridge routes, each under its own conferral row.
//!
//! Each holds one sealed kernel seed privately and answers exactly the
//! questions the bridge routes ask of it:
//!
//! | view | built from | can do | cannot do |
//! |---|---|---|---|
//! | [`GatewayManifestSigner`] | `SigningKeys` | seal a bridge manifest (canonical payload signed with the manifest key); publish that key's public half | mint or verify a JWT, read the RSA signing key, rotate a key |
//! | [`BridgeReleaseSource`] | the secret lookup | read the one custom secret a bridge release spec names as its token | read a built-in secret (signing keys, pepper, database URLs, provider keys), or any secret by a name it chooses |
//! | [`GatewaySettingsView`] | `Config`, the system admin, the services root, the secret lookup | answer the external URL, the API-key lifetime, the trusted proxies, the organisation owner, the services root, and whether a named custom secret or provider key is configured | return a secret value, answer for a signing key, the pepper or a database URL, change any setting |
//! | [`BridgeEventStream`] | the kernel's `EventRouter` | open a per-user `AgUI` subscription with its connection guard | publish an event, reach another broadcaster |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::response::sse::Event;
use systemprompt_events::{AgUiBroadcaster, Broadcaster, ConnectionGuard, EventRouter};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_manifest::{Config, Secrets};
use systemprompt_models::AgUiEvent;
use systemprompt_models::bridge::ids::ManifestSignature;
use systemprompt_models::bridge::manifest::{SignedManifest, SignedManifestEnvelope};
use systemprompt_security::keys::SigningKeys;
use systemprompt_security::{ManifestSigningError, manifest_signing};
use systemprompt_traits::SecretLookup;
use tokio::sync::mpsc;

const SUBSCRIPTION_BUFFER: usize = 1024;

/// Signs bridge manifests with the manifest key and publishes its public half.
#[derive(Clone)]
pub struct GatewayManifestSigner(SigningKeys);

impl GatewayManifestSigner {
    #[must_use]
    pub const fn new(keys: SigningKeys) -> Self {
        Self(keys)
    }

    pub fn seal(
        &self,
        manifest: &SignedManifest,
    ) -> Result<SignedManifestEnvelope, ManifestSigningError> {
        let payload = manifest_signing::canonicalize(manifest)?;
        let signature = self.0.sign_manifest_bytes(payload.as_bytes());
        Ok(SignedManifestEnvelope {
            payload,
            signature: ManifestSignature::new(signature),
        })
    }

    #[must_use]
    pub fn public_key_b64(&self) -> String {
        self.0.manifest_pubkey_b64()
    }
}

impl std::fmt::Debug for GatewayManifestSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayManifestSigner")
            .finish_non_exhaustive()
    }
}

/// Reads the GitHub token a bridge release spec names, and nothing else.
#[derive(Clone)]
pub struct BridgeReleaseSource(Arc<dyn SecretLookup>);

impl BridgeReleaseSource {
    #[must_use]
    pub fn new(lookup: Arc<dyn SecretLookup>) -> Self {
        Self(lookup)
    }

    #[must_use]
    pub fn token(&self, spec: &BridgeReleasesSpec) -> Option<String> {
        let name = spec
            .token_secret
            .as_deref()
            .filter(|name| !Secrets::is_built_in_name(name))?;
        self.0
            .lookup(name)
            .map(|secret| secret.expose().to_owned())
            .filter(|token| !token.is_empty())
    }
}

impl std::fmt::Debug for BridgeReleaseSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeReleaseSource")
            .finish_non_exhaustive()
    }
}

/// The configuration facts the bridge routes read.
#[derive(Clone)]
pub struct GatewaySettingsView {
    api_external_url: String,
    api_key_max_lifetime_days: Option<u32>,
    trusted_proxies: Vec<ipnet::IpNet>,
    system_admin: UserId,
    services_root: PathBuf,
    secrets: Arc<dyn SecretLookup>,
}

/// What a [`GatewaySettingsView`] is built from.
#[derive(Debug)]
pub struct GatewaySettingsSource<'a> {
    pub config: &'a Config,
    pub system_admin: UserId,
    pub services_root: PathBuf,
    pub secrets: Arc<dyn SecretLookup>,
}

impl GatewaySettingsView {
    #[must_use]
    pub fn new(source: GatewaySettingsSource<'_>) -> Self {
        Self {
            api_external_url: source.config.api_external_url.clone(),
            api_key_max_lifetime_days: source.config.api_key_max_lifetime_days,
            trusted_proxies: source.config.trusted_proxies.clone(),
            system_admin: source.system_admin,
            services_root: source.services_root,
            secrets: source.secrets,
        }
    }

    #[must_use]
    pub fn api_external_url(&self) -> &str {
        &self.api_external_url
    }

    #[must_use]
    pub const fn api_key_max_lifetime_days(&self) -> Option<u32> {
        self.api_key_max_lifetime_days
    }

    #[must_use]
    pub fn trusted_proxies(&self) -> &[ipnet::IpNet] {
        &self.trusted_proxies
    }

    #[must_use]
    pub const fn system_admin(&self) -> &UserId {
        &self.system_admin
    }

    #[must_use]
    pub fn services_root(&self) -> &Path {
        &self.services_root
    }

    #[must_use]
    pub fn credential_configured(&self, name: &str) -> bool {
        (!Secrets::is_built_in_name(name) || Secrets::is_provider_key_name(name))
            && self
                .secrets
                .lookup(name)
                .is_some_and(|secret| !secret.expose().is_empty())
    }
}

impl std::fmt::Debug for GatewaySettingsView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewaySettingsView")
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

/// Opens per-user subscriptions to the `AgUI` event broadcaster.
#[derive(Clone)]
pub struct BridgeEventStream(Arc<AgUiBroadcaster>);

impl BridgeEventStream {
    #[must_use]
    pub fn new(router: &EventRouter) -> Self {
        Self(Arc::clone(&router.broadcasters().agui))
    }

    pub async fn subscribe(&self, user: &UserId) -> Option<BridgeEventSubscription> {
        let connection = ConnectionId::generate();
        let (tx, rx) = mpsc::channel(SUBSCRIPTION_BUFFER);
        if !self.0.register(user, &connection, tx).await {
            return None;
        }
        Some(BridgeEventSubscription {
            receiver: rx,
            _guard: ConnectionGuard::new(Arc::clone(&self.0), user.clone(), connection),
        })
    }
}

impl std::fmt::Debug for BridgeEventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeEventStream").finish_non_exhaustive()
    }
}

/// One open subscription; dropping it deregisters the connection.
#[derive(Debug)]
pub struct BridgeEventSubscription {
    receiver: mpsc::Receiver<Result<Event, Infallible>>,
    _guard: ConnectionGuard<AgUiEvent>,
}

impl futures_util::Stream for BridgeEventSubscription {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}
