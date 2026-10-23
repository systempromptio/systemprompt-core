//! How every credential exchange talks HTTP; the client itself is owned by
//! the [`super::cache::TokenCache`] the exchange mints into.
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

use std::sync::Arc;
use std::time::Duration;

use systemprompt_models::net::{HTTP_AUTH_VERIFY_TIMEOUT, HTTP_CONNECT_TIMEOUT};

use super::error::CredentialError;

const RETRY_BACKOFF: Duration = Duration::from_millis(250);

pub(super) fn build_client() -> Result<reqwest::Client, Arc<reqwest::Error>> {
    reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_AUTH_VERIFY_TIMEOUT)
        .build()
        .map_err(Arc::new)
}

pub(crate) async fn post_form(
    client: &reqwest::Client,
    uri: &str,
    form: &[(&str, String)],
) -> Result<String, CredentialError> {
    send_with_retry(client, uri, |client| client.post(uri).form(form)).await
}

pub(crate) async fn post_bearer(
    client: &reqwest::Client,
    uri: &str,
    bearer: &str,
    headers: &[(&str, &str)],
) -> Result<String, CredentialError> {
    send_with_retry(client, uri, |client| {
        headers.iter().fold(
            client.post(uri).bearer_auth(bearer),
            |request, (name, value)| request.header(*name, *value),
        )
    })
    .await
}

async fn send_with_retry<B>(
    client: &reqwest::Client,
    uri: &str,
    build: B,
) -> Result<String, CredentialError>
where
    B: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
{
    let mut attempt = 0;
    loop {
        let outcome = attempt_send(build(client)).await;
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
            Err(Retryable::Transport(source)) => Err(CredentialError::Unreachable {
                uri: uri.to_owned(),
                source,
            }),
            Err(Retryable::Unavailable { status, body } | Retryable::Refused { status, body }) => {
                Err(CredentialError::Rejected { status, body })
            },
        };
    }
}

enum Retryable {
    Transport(reqwest::Error),
    Unavailable { status: String, body: String },
    Refused { status: String, body: String },
}

async fn attempt_send(request: reqwest::RequestBuilder) -> Result<String, Retryable> {
    let response = request.send().await.map_err(Retryable::Transport)?;

    let status = response.status();
    let body = response.text().await.map_err(Retryable::Transport)?;
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
