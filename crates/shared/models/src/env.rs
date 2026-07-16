//! Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation.
//!
//! A single primitive shared by every config surface that expands placeholders:
//! the profile loader interpolates a whole YAML document against the process
//! environment, and the services config layer drives [`interpolate`] in a
//! multi-pass loop over a secrets→env→vars source chain. Both reuse the one
//! regex and the one unresolved-placeholder rule defined here, so the syntax
//! never drifts between surfaces.

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

/// Reads an environment variable, treating empty as absent.
///
/// Returns `Some` only when the variable is present and non-empty, so a
/// blank override never masks a downstream default.
#[must_use]
pub fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}

/// Normalizes an optional env- or flag-sourced value, treating blank as absent.
///
/// clap `env =` args materialize as `Some("")` when the variable is exported
/// empty — container platforms export blank defaults for unfilled template
/// variables — so presence checks must not read that as a configured value.
#[must_use]
pub fn none_if_blank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Reports whether `input` still contains a `${VAR}` / `${VAR:-default}`
/// placeholder. Used by multi-pass resolvers to detect non-convergence.
#[must_use]
pub fn contains_placeholder(input: &str) -> bool {
    INTERPOLATION_REGEX.is_match(input)
}

/// Replaces every `${VAR}` / `${VAR:-default}` occurrence in `input` using
/// `lookup`.
///
/// Resolution order per placeholder: `lookup(var)`, then the inline `:-default`
/// if present, otherwise the literal placeholder is left untouched. A single
/// pass; transitive resolution (a resolved value that itself contains a
/// placeholder) is the caller's concern.
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
