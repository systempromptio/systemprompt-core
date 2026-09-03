//! Cowork egress allowlist resolution for the MDM payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const LOOPBACK_ALIAS: &str = "loopback";
const LOOPBACK_HOST: &str = "127.0.0.1";
const ENV_SUFFIX: &str = "EGRESS_ALLOWED_HOSTS";

#[must_use]
pub fn parse_egress_allowed_hosts(raw: &str) -> Option<Vec<String>> {
    parse(raw)
}

#[must_use]
pub fn cowork_egress_allowed_hosts(from_flag: Option<&[String]>) -> Option<Vec<String>> {
    if let Some(hosts) = from_flag {
        return Some(hosts.to_vec());
    }
    std::env::var(crate::brand::brand().env(ENV_SUFFIX))
        .ok()
        .as_deref()
        .and_then(parse)
}

fn parse(raw: &str) -> Option<Vec<String>> {
    let hosts: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(|h| {
            if h.eq_ignore_ascii_case(LOOPBACK_ALIAS) {
                LOOPBACK_HOST.to_owned()
            } else {
                h.to_owned()
            }
        })
        .collect();
    (!hosts.is_empty()).then_some(hosts)
}
