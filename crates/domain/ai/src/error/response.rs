//! Building [`AiError`] from an upstream HTTP response and classifying it for
//! retry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::resilience::Outcome;

use super::AiError;

impl AiError {
    pub async fn from_error_response(provider: &str, response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let retry_after = parse_retry_after(response.headers());
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<unreadable body: {e}>"));
        Self::HttpStatus {
            provider: provider.to_owned(),
            status,
            retry_after,
            body,
        }
    }

    #[must_use]
    pub fn classify(&self) -> Outcome {
        match self {
            Self::HttpStatus {
                status,
                retry_after,
                ..
            } => {
                if matches!(*status, 408 | 425 | 429 | 500 | 502 | 503 | 504) {
                    Outcome::Transient {
                        retry_after: *retry_after,
                    }
                } else {
                    Outcome::Permanent
                }
            },
            Self::RateLimit { .. } | Self::Timeout { .. } => {
                Outcome::Transient { retry_after: None }
            },
            Self::Http(err) if err.is_timeout() || err.is_connect() => {
                Outcome::Transient { retry_after: None }
            },
            _ => Outcome::Permanent,
        }
    }
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}
