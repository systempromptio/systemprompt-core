//! Transport for Vault / `OpenBao` HTTP calls.
//!
//! Redirects are refused rather than followed: a 3xx from a compromised or
//! misconfigured Vault would otherwise replay the `X-Vault-Token` header at an
//! attacker-chosen host. Retries cover only connect failures, 429 and 5xx —
//! any other 4xx is a decision Vault has already made and repeating it would
//! burn the `AppRole` secret id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use reqwest::redirect::Policy;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use systemprompt_models::net::{trusted_http_hosts_from_env, validate_outbound_url_with_trust};
use systemprompt_models::profile::VaultSecretsConfig;

use super::error::VaultError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_BASE_DELAY: Duration = Duration::from_millis(200);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) struct VaultHttp {
    client: reqwest::Client,
    address: url::Url,
    namespace: Option<String>,
    retries: u8,
}

impl VaultHttp {
    pub(super) fn new(cfg: &VaultSecretsConfig) -> Result<Self, VaultError> {
        let trusted = trusted_http_hosts_from_env();
        let address = validate_outbound_url_with_trust(&cfg.address, &trusted).map_err(|e| {
            VaultError::Address {
                message: e.to_string(),
            }
        })?;

        let mut builder = reqwest::Client::builder()
            .use_rustls_tls()
            .redirect(Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(Duration::from_secs(cfg.timeout_secs));

        if let Some(path) = cfg.ca_cert_path.as_deref() {
            let pem = std::fs::read(path).map_err(|e| VaultError::CaCertificate {
                path: path.to_owned(),
                message: e.to_string(),
            })?;
            let cert =
                reqwest::Certificate::from_pem(&pem).map_err(|e| VaultError::CaCertificate {
                    path: path.to_owned(),
                    message: e.to_string(),
                })?;
            builder = builder.add_root_certificate(cert);
        }

        let client = builder.build().map_err(|e| VaultError::ClientBuild {
            message: e.to_string(),
        })?;

        Ok(Self {
            client,
            address,
            namespace: cfg.namespace.clone(),
            retries: cfg.retries,
        })
    }

    pub(super) fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, VaultError> {
        let url = self.address.join(path).map_err(|e| VaultError::Address {
            message: e.to_string(),
        })?;
        let mut builder = self
            .client
            .request(method, url)
            .header("X-Vault-Request", "true");
        if let Some(namespace) = self.namespace.as_deref() {
            builder = builder.header("X-Vault-Namespace", namespace);
        }
        Ok(builder)
    }

    pub(super) async fn send_with_retry<F>(&self, build: F) -> Result<Response, VaultError>
    where
        F: Fn() -> Result<RequestBuilder, VaultError>,
    {
        let attempts = u32::from(self.retries).max(1);
        let mut last = String::new();

        for attempt in 0..attempts {
            if attempt > 0 {
                tokio::time::sleep(backoff_delay(attempt)).await;
            }
            match build()?.send().await {
                Ok(response) if is_retryable(response.status()) => {
                    last = format!("HTTP {}", response.status().as_u16());
                },
                Ok(response) => return Ok(response),
                Err(e) if e.is_connect() || e.is_timeout() => last = transport_message(&e),
                Err(e) => {
                    return Err(VaultError::Exhausted {
                        attempts: attempt + 1,
                        message: transport_message(&e),
                    });
                },
            }
            if attempt + 1 < attempts {
                tracing::warn!(
                    attempt = attempt + 1,
                    reason = %last,
                    "vault request failed, retrying"
                );
            }
        }

        Err(VaultError::Exhausted {
            attempts,
            message: last,
        })
    }
}

fn transport_message(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "request timed out".to_owned()
    } else {
        "could not connect".to_owned()
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn backoff_delay(attempt: u32) -> Duration {
    RETRY_BASE_DELAY
        .saturating_mul(1u32 << attempt.min(5))
        .min(RETRY_MAX_DELAY)
}
