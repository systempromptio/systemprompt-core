//! Cowork egress allowlist resolution for the MDM payloads.
//!
//! An allowlist that is supplied but names no host is a configuration error,
//! never "unrestricted". When neither the flag nor the environment supplies
//! a policy, explicitly allow public web access with `*`; Desktop otherwise
//! permits only the inference endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const LOOPBACK_ALIAS: &str = "loopback";
const LOOPBACK_HOST: &str = "127.0.0.1";
const ENV_SUFFIX: &str = "EGRESS_ALLOWED_HOSTS";

#[derive(Debug, thiserror::Error)]
pub enum EgressParseError {
    #[error("egress allow-list {raw:?} names no host")]
    Empty { raw: String },
}

pub fn parse_egress_allowed_hosts(raw: &str) -> Result<Vec<String>, EgressParseError> {
    parse(raw)
}

pub fn cowork_egress_allowed_hosts(
    from_flag: Option<&[String]>,
) -> Result<Option<Vec<String>>, EgressParseError> {
    if let Some(hosts) = from_flag {
        if hosts.is_empty() {
            return Err(EgressParseError::Empty { raw: String::new() });
        }
        return Ok(Some(hosts.to_vec()));
    }
    std::env::var(crate::brand::brand().env(ENV_SUFFIX))
        .ok()
        .as_deref()
        .map(parse)
        .transpose()
        .map(|hosts| Some(hosts.unwrap_or_else(|| vec!["*".to_owned()])))
}

fn parse(raw: &str) -> Result<Vec<String>, EgressParseError> {
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
    if hosts.is_empty() {
        return Err(EgressParseError::Empty {
            raw: raw.to_owned(),
        });
    }
    Ok(hosts)
}
