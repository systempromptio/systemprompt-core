//! How one operator compares one value.
//!
//! Split from [`super::rules`] because the two answer different questions: that
//! module decides which calls a rule covers and how its conditions combine,
//! this one decides whether a single argument satisfies a single test. The
//! address and domain handling in particular is security-relevant on its own
//! terms and is easier to review, and to test, away from the config plumbing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Op {
    Equals,
    Contains,
    Prefix,
    Suffix,
    Glob,
    DomainSuffix,
    Gt,
    Gte,
    Lt,
    Lte,
    Exists,
}

impl Op {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::Contains => "contains",
            Self::Prefix => "prefix",
            Self::Suffix => "suffix",
            Self::Glob => "glob",
            Self::DomainSuffix => "domain_suffix",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Exists => "exists",
        }
    }

    pub(super) const fn is_numeric(self) -> bool {
        matches!(self, Self::Gt | Self::Gte | Self::Lt | Self::Lte)
    }

    pub(super) fn test(
        self,
        value: &serde_json::Value,
        strings: &[String],
        number: Option<f64>,
    ) -> Option<bool> {
        if self == Self::Exists {
            return Some(true);
        }
        if self.is_numeric() {
            let (found, want) = (value.as_f64()?, number?);
            return Some(match self {
                Self::Gt => found > want,
                Self::Gte => found >= want,
                Self::Lt => found < want,
                _ => found <= want,
            });
        }
        let text = value.as_str()?;
        Some(strings.iter().any(|want| match self {
            Self::Equals => text.eq_ignore_ascii_case(want),
            Self::Contains => lower(text).contains(&lower(want)),
            Self::Prefix => lower(text).starts_with(&lower(want)),
            Self::Suffix => lower(text).ends_with(&lower(want)),
            Self::Glob => glob_match(&lower(want), &lower(text)),
            _ => every_addr_domain_matches(text, want),
        }))
    }
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

// Why: a single `to` field routinely carries a comma-joined recipient list, and
// `addr_domain` reduces it to the LAST address. Judging the field by that one
// address lets "a@evil.com, b@ours.io" pass a negated domain_suffix rule. Every
// address must match, and a list that parses to nothing does not match.
fn every_addr_domain_matches(text: &str, want: &str) -> bool {
    let mut seen = false;
    let all = text
        .split([',', ';'])
        .map(str::trim)
        .filter(|addr| !addr.is_empty())
        .all(|addr| {
            seen = true;
            addr_domain(addr).is_some_and(|domain| domain_matches(domain, want))
        });
    seen && all
}

fn addr_domain(raw: &str) -> Option<&str> {
    let spec = raw.rfind('<').map_or(raw, |i| &raw[i + 1..]);
    let spec = spec.strip_suffix('>').unwrap_or(spec).trim();
    let at = spec.rfind('@')?;
    let domain = spec[at + 1..].trim();
    (!domain.is_empty()).then_some(domain)
}

fn domain_matches(domain: &str, want: &str) -> bool {
    let domain = lower(domain);
    let want = lower(want.trim_start_matches('.'));
    domain == want || domain.ends_with(&format!(".{want}"))
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

pub(super) fn erase_indices(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut depth = 0u32;
    for ch in path.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {},
        }
    }
    out
}
