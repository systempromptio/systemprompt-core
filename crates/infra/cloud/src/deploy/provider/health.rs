//! Tenant readiness read from the instance's own `/readyz` probe.
//!
//! `/readyz` answers 200 only when the instance admits traffic, and its body
//! carries the running core version on every status, so one probe reports
//! both. A probe that cannot reach the instance is an unready result, not an
//! error: the caller is usually polling a rollout that has not finished.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{CloudError, CloudResult};

const READYZ_PATH: &str = "/readyz";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TenantHealth {
    pub url: String,
    pub http_status: Option<u16>,
    pub ready: bool,
    pub status: String,
    pub version: Option<String>,
}

#[derive(Deserialize)]
struct ReadyzBody {
    status: String,
    #[serde(default)]
    version: Option<String>,
}

#[must_use]
pub fn tenant_base_url(hostname: &str) -> String {
    if hostname.starts_with("http://") || hostname.starts_with("https://") {
        hostname.trim_end_matches('/').to_owned()
    } else {
        format!("https://{}", hostname.trim_end_matches('/'))
    }
}

pub async fn probe_tenant(client: &reqwest::Client, base_url: &str) -> TenantHealth {
    let url = format!("{base_url}{READYZ_PATH}");
    let response = match client.get(&url).send().await {
        Ok(response) => response,
        Err(error) => {
            return TenantHealth {
                url: base_url.to_owned(),
                http_status: None,
                ready: false,
                status: format!("unreachable: {error}"),
                version: None,
            };
        },
    };
    let http_status = response.status();
    let body = response.json::<ReadyzBody>().await;
    match body {
        Ok(body) => TenantHealth {
            url: base_url.to_owned(),
            http_status: Some(http_status.as_u16()),
            ready: http_status.is_success() && body.status == "ready",
            status: body.status,
            version: body.version,
        },
        Err(error) => TenantHealth {
            url: base_url.to_owned(),
            http_status: Some(http_status.as_u16()),
            ready: false,
            status: format!("unreadable /readyz body: {error}"),
            version: None,
        },
    }
}

pub async fn wait_until_ready(
    client: &reqwest::Client,
    base_url: &str,
    timeout: Duration,
    interval: Duration,
) -> CloudResult<TenantHealth> {
    let started = Instant::now();
    loop {
        let health = probe_tenant(client, base_url).await;
        if health.ready {
            return Ok(health);
        }
        if started.elapsed() + interval > timeout {
            return Err(CloudError::TenantUnready {
                url: base_url.to_owned(),
                waited_secs: started.elapsed().as_secs(),
                last: health.status,
            });
        }
        tokio::time::sleep(interval).await;
    }
}
