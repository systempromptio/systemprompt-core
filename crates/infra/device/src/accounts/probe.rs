//! Probing an address: whether it is a systemprompt instance this build can
//! sign in to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::SpClient;

use crate::accounts::accounts::Accounts;

/// What an address turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    Malformed,
    Unreachable,
    Unsupported,
    Instance(ProbedInstance),
}

/// A systemprompt instance that answered a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedInstance {
    pub url: String,
    pub host: String,
    pub label: String,
    pub version: Option<String>,
    pub modes: Vec<String>,
    pub browser: bool,
    pub sign_in_hint: Option<String>,
}

impl Accounts {
    pub async fn probe(&self, url: &str) -> ProbeResult {
        let Ok(client) = SpClient::new(url, self.inner.http.clone()) else {
            return ProbeResult::Malformed;
        };
        match client.probe().await {
            Ok(found) => ProbeResult::Instance(ProbedInstance {
                url: client.base().to_owned(),
                host: host_of(&client),
                label: label_for(found.discovery.display_name.as_deref(), &client),
                version: found.version,
                modes: found.capabilities.modes,
                browser: found.discovery.discovered,
                sign_in_hint: found.discovery.sign_in_hint,
            }),
            Err(error) if error.is_transient() => {
                tracing::debug!(error = %error, "probe: no answer");
                ProbeResult::Unreachable
            },
            Err(error) => {
                tracing::debug!(error = %error, "probe: not a supported instance");
                ProbeResult::Unsupported
            },
        }
    }
}

pub(crate) fn host_of(client: &SpClient) -> String {
    let origin = client.origin();
    match (origin.host_str(), origin.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        (None, _) => client.base().to_owned(),
    }
}

pub(crate) fn label_for(display_name: Option<&str>, client: &SpClient) -> String {
    display_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(|| host_of(client), str::to_owned)
}
