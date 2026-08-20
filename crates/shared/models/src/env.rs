//! Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation.
//!
//! A single primitive shared by every config surface that expands placeholders:
//! the profile loader interpolates a whole YAML document against the process
//! environment, and the services config layer drives [`interpolate`] in a
//! multi-pass loop over a secrets→env→vars source chain. Both reuse the one
//! regex and the one unresolved-placeholder rule defined here, so the syntax
//! never drifts between surfaces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

use regex::Regex;

#[expect(
    clippy::expect_used,
    reason = "compile-time-constant regex; failure is a programmer bug, not runtime input"
)]
static INTERPOLATION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}:]+)(?::-(.*?))?\}")
        .expect("INTERPOLATION_REGEX is a valid regex - this is a compile-time constant")
});

#[must_use]
pub fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}

#[must_use]
pub fn none_if_blank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

#[must_use]
pub fn contains_placeholder(input: &str) -> bool {
    INTERPOLATION_REGEX.is_match(input)
}

#[must_use]
pub fn interpolate(input: &str, lookup: &impl Fn(&str) -> Option<String>) -> String {
    INTERPOLATION_REGEX
        .replace_all(input, |caps: &regex::Captures| {
            let full = caps[0].to_owned();
            let var_name = &caps[1];
            let default_value = caps.get(2).map(|m| m.as_str());
            lookup(var_name).unwrap_or_else(|| default_value.map_or(full, str::to_owned))
        })
        .into_owned()
}
