//! HTTP API client for systemprompt.io deployments.
//!
//! The crate exposes a single type, [`SystempromptClient`], which wraps a
//! pre-configured [`reqwest::Client`] and a small set of typed methods for the
//! routes documented in `systemprompt-models::ApiPaths`. All errors flow
//! through the [`ClientError`] enum.
//!
//! # Feature flags
//!
//! This crate has no feature flags. `[package.metadata.docs.rs] all-features`
//! is set so future additions render on docs.rs without further changes.
//!
//! # Example
//!
//! ```no_run
//! use systemprompt_client::SystempromptClient;
//!
//! # async fn run() -> systemprompt_client::ClientResult<()> {
//! let client = SystempromptClient::new("https://api.example.com")?;
//! let healthy = client.check_health().await;
//! assert!(healthy);
//! # Ok(()) }
//! ```

mod client;
mod error;

pub use client::SystempromptClient;
pub use error::{ClientError, ClientResult};
