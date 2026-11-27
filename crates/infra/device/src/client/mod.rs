//! HTTP client for one systemprompt instance.
//!
//! [`SpClient`] speaks every route the harness uses: discovery and the health
//! probe, the device-link sign-in and its code exchange, PAT to JWT sessions,
//! the signed manifest, profile, model catalog, identity and plugin bundle
//! files, the governance hooks (hook client, hook token, track, govern), the
//! device-authenticated consumer API and the heartbeat. It is stateless about
//! credentials: every call takes the credential it needs, and nothing here
//! stores, caches or logs one.
//!
//! Credentials only ever travel to the instance's own origin. The base URL must
//! be https, or plain http on loopback; every path is resolved with the
//! same-origin rules of [`crate::protocol::origin`], and redirects are never
//! followed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod consumer;
pub mod credentials;
pub mod error;
pub mod hooks;
pub mod http;
mod instance;
pub mod loopback;
pub mod manifest;
mod reads;
pub mod signin;

pub use consumer::ConsumerOutcome;
pub use credentials::{CredentialError, CredentialSource, DeviceTokenSource, Freshness, with_jwt};
pub use error::{ClientError, Incompatibility, TransportFailure};
pub use hooks::{HookClient, HookGrant};
pub use http::{HttpClient, HttpClientError};
pub use instance::SpClient;
pub use loopback::{CallbackOutcome, LoopbackCallback, LoopbackError};
pub use manifest::{ManifestFetch, ManifestKey, open_manifest, verify};
pub use signin::{InstanceDiscovery, InstanceProbe, JwtSession};
