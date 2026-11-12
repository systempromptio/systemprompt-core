//! The operator's forward-proxy environment, read in one place.
//!
//! `HTTP(S)_PROXY`, `ALL_PROXY` and `NO_PROXY` (either case) are what reqwest
//! and curl honour. [`ProxyEnvironment::from_env`] is the only reader; every
//! decision over the parsed value is a pure method, so the guarded client and
//! the sandboxed git path share one interpretation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const ALL_PROXY_ENV: [&str; 6] = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
];
const HTTPS_PROXY_ENV: [&str; 4] = ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"];
const NO_PROXY_ENV: [&str; 2] = ["NO_PROXY", "no_proxy"];

/// Parsed forward-proxy settings: every proxy host, the proxy an `https`
/// request uses, and the `NO_PROXY` bypass list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyEnvironment {
    proxy_hosts: Vec<String>,
    https_proxy: Option<url::Url>,
    no_proxy: String,
}

impl ProxyEnvironment {
    #[must_use]
    pub fn new(proxies: &[&str], https_proxy: Option<&str>, no_proxy: &str) -> Self {
        Self {
            proxy_hosts: proxies.iter().filter_map(|raw| proxy_host(raw)).collect(),
            https_proxy: https_proxy.and_then(parse_proxy),
            no_proxy: no_proxy.to_owned(),
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let read = |names: &[&str]| -> Vec<String> {
            names
                .iter()
                .filter_map(|name| std::env::var(name).ok())
                .filter(|raw| !raw.trim().is_empty())
                .collect()
        };
        let proxies = read(&ALL_PROXY_ENV);
        let https = read(&HTTPS_PROXY_ENV);
        let no_proxy = read(&NO_PROXY_ENV);
        let proxies: Vec<&str> = proxies.iter().map(String::as_str).collect();
        Self::new(
            &proxies,
            https.first().map(String::as_str),
            no_proxy.first().map_or("", String::as_str),
        )
    }

    #[must_use]
    pub fn proxy_hosts(&self) -> &[String] {
        &self.proxy_hosts
    }

    #[must_use]
    pub fn bypasses(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        self.no_proxy
            .split(',')
            .map(|entry| entry.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|entry| !entry.is_empty())
            .any(|entry| entry == "*" || host == entry || host.ends_with(&format!(".{entry}")))
    }

    #[must_use]
    pub fn https_proxy_for(&self, host: &str) -> Option<&url::Url> {
        self.https_proxy.as_ref().filter(|_| !self.bypasses(host))
    }
}

fn parse_proxy(raw: &str) -> Option<url::Url> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.contains("://") {
        url::Url::parse(raw).ok()
    } else {
        url::Url::parse(&format!("http://{raw}")).ok()
    }
}

#[must_use]
pub fn proxy_host(raw: &str) -> Option<String> {
    parse_proxy(raw)?.host_str().map(str::to_ascii_lowercase)
}
