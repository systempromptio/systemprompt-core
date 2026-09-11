//! The one HTTP client every credential exchange uses.
//!
//! Token exchange sits in front of a user's request, so it is bounded twice —
//! a connect timeout and a total timeout — rather than left to inherit
//! reqwest's default of none. A hung identity provider must cost a request,
//! not a worker.
//!
//! One retry, and only one. A connect failure or a 5xx is the identity
//! provider being briefly unavailable, which is the case a single retry fixes;
//! a 4xx is a credential an operator has to change, and retrying it only
//! doubles the rate-limit pressure on a key that is already refused. The
//! backoff is fixed and un-jittered: with at most one retry there is no
//! thundering herd for jitter to spread out.
//!
//! The response body is returned as text rather than parsed here because each
//! credential type reads a different response shape out of it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;
use std::time::Duration;

use systemprompt_models::net::{HTTP_AUTH_VERIFY_TIMEOUT, HTTP_CONNECT_TIMEOUT};

use super::error::CredentialError;

const RETRY_BACKOFF: Duration = Duration::from_millis(250);

fn client() -> Result<&'static reqwest::Client, CredentialError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_AUTH_VERIFY_TIMEOUT)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| CredentialError::Client(e.clone()))
}

pub(crate) async fn post_form(
    uri: &str,
    form: &[(&str, String)],
) -> Result<String, CredentialError> {
    let client = client()?;
    let mut attempt = 0;
    loop {
        let outcome = attempt_post(client, uri, form).await;
        let retryable = matches!(
            outcome,
            Err(Retryable::Transport(_) | Retryable::Unavailable { .. })
        );
        if retryable && attempt == 0 {
            attempt += 1;
            tokio::time::sleep(RETRY_BACKOFF).await;
            continue;
        }
        return match outcome {
            Ok(body) => Ok(body),
            Err(Retryable::Transport(reason)) => Err(CredentialError::Unreachable {
                uri: uri.to_owned(),
                reason,
            }),
            Err(Retryable::Unavailable { status, body } | Retryable::Refused { status, body }) => {
                Err(CredentialError::Rejected { status, body })
            },
        };
    }
}

enum Retryable {
    Transport(String),
    Unavailable { status: String, body: String },
    Refused { status: String, body: String },
}

async fn attempt_post(
    client: &reqwest::Client,
    uri: &str,
    form: &[(&str, String)],
) -> Result<String, Retryable> {
    let response = client
        .post(uri)
        .form(form)
        .send()
        .await
        .map_err(|e| Retryable::Transport(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        return Ok(body);
    }
    let status_text = status.to_string();
    let body = body.trim().to_owned();
    if status.is_server_error() {
        return Err(Retryable::Unavailable {
            status: status_text,
            body,
        });
    }
    Err(Retryable::Refused {
        status: status_text,
        body,
    })
}
