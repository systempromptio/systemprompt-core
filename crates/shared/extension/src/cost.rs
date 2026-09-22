//! The measured cost a migration declares about its own bulk work.
//!
//! A migration that rewrites rows in a hot table is the one thing that can
//! turn a deploy into an unexplained outage: it runs inside the boot, before
//! the HTTP listener is bound, in one transaction, holding its locks, fanning
//! out through every per-row trigger on the table. A 3,644-row `UPDATE
//! ai_requests` measured 113 ms per row on a production instance — 27 minutes
//! of silence — because one trigger re-enqueued the whole client session per
//! row.
//!
//! The directive is how an author states what they measured. `rows` is what
//! the migration actually wrote and `measured` is the wall clock of its
//! **slowest single statement** — `statement_timeout` is enforced per
//! statement, so that is the number the bound has to cover:
//!
//! ```sql
//! -- @cost: rows=3644 measured=2.0s triggers=suspended
//! ```
//!
//! It must sit in the leading comment block, before any SQL, like
//! `@no-transaction` and `@supersedes-checksum`. Its value is load-bearing in
//! two places: the build fails on a malformed one, and the runner derives the
//! migration's `statement_timeout` from `measured`, so a statement that runs
//! far longer than its author measured is cancelled instead of hanging a
//! customer's boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

/// Whether the author suspended the table's per-row triggers for the bulk
/// statement. `live` is a deliberate declaration, not a default: it says the
/// fan-out was considered and is wanted (a correction the projections must
/// see), rather than overlooked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerPolicy {
    Suspended,
    Live,
}

impl TriggerPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Suspended => "suspended",
            Self::Live => "live",
        }
    }
}

/// A parsed `-- @cost:` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostDirective {
    /// Rows the migration wrote, from a real run.
    pub rows: u64,
    /// Wall clock of the slowest single statement in that run.
    pub measured: Duration,
    pub triggers: TriggerPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CostDirectiveError {
    #[error("`@cost` must name {0}")]
    Missing(&'static str),
    #[error("`@cost` field `{0}` is not `key=value`")]
    NotAPair(String),
    #[error("`@cost` does not take `{0}`; it takes rows, measured and triggers")]
    UnknownField(String),
    #[error("`@cost` names `{0}` twice")]
    Duplicate(&'static str),
    #[error("`@cost` rows must be a count, got `{0}`")]
    Rows(String),
    #[error("`@cost` measured must be a duration like `250ms`, `2.0s` or `3m`, got `{0}`")]
    Measured(String),
    #[error("`@cost` triggers must be `suspended` or `live`, got `{0}`")]
    Triggers(String),
    #[error("`@cost` is declared {0} times; one migration states its cost once")]
    Repeated(usize),
}

const DIRECTIVE: &str = "-- @cost:";

/// Reads the directive out of a migration body.
///
/// `Ok(None)` means the migration does not declare a cost, which is correct
/// for the great majority — only bulk work on a hot table needs one, and that
/// requirement is enforced by the migration-cost gate, not here.
pub fn parse(sql: &str) -> Result<Option<CostDirective>, CostDirectiveError> {
    let lines: Vec<&str> = sql
        .lines()
        .map(str::trim)
        .take_while(|line| line.is_empty() || line.starts_with("--"))
        .filter_map(|line| line.strip_prefix(DIRECTIVE))
        .collect();
    match lines.as_slice() {
        [] => Ok(None),
        [single] => parse_fields(single).map(Some),
        many => Err(CostDirectiveError::Repeated(many.len())),
    }
}

fn parse_fields(rest: &str) -> Result<CostDirective, CostDirectiveError> {
    let mut rows: Option<u64> = None;
    let mut measured: Option<Duration> = None;
    let mut triggers: Option<TriggerPolicy> = None;
    for field in rest.split_whitespace() {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| CostDirectiveError::NotAPair(field.to_owned()))?;
        match key {
            "rows" => {
                if rows.is_some() {
                    return Err(CostDirectiveError::Duplicate("rows"));
                }
                rows = Some(
                    value
                        .parse()
                        .map_err(|_| CostDirectiveError::Rows(value.to_owned()))?,
                );
            },
            "measured" => {
                if measured.is_some() {
                    return Err(CostDirectiveError::Duplicate("measured"));
                }
                measured = Some(parse_duration(value)?);
            },
            "triggers" => {
                if triggers.is_some() {
                    return Err(CostDirectiveError::Duplicate("triggers"));
                }
                triggers = Some(match value {
                    "suspended" => TriggerPolicy::Suspended,
                    "live" => TriggerPolicy::Live,
                    other => return Err(CostDirectiveError::Triggers(other.to_owned())),
                });
            },
            other => return Err(CostDirectiveError::UnknownField(other.to_owned())),
        }
    }
    Ok(CostDirective {
        rows: rows.ok_or(CostDirectiveError::Missing("rows"))?,
        measured: measured.ok_or(CostDirectiveError::Missing("measured"))?,
        triggers: triggers.ok_or(CostDirectiveError::Missing("triggers"))?,
    })
}

// Why: the unit is part of the value so the number is never ambiguous in
// review; a bare `2` could be seconds or milliseconds depending on the reader.
fn parse_duration(value: &str) -> Result<Duration, CostDirectiveError> {
    let malformed = || CostDirectiveError::Measured(value.to_owned());
    let (number, multiplier) = if let Some(rest) = value.strip_suffix("ms") {
        (rest, 1.0)
    } else if let Some(rest) = value.strip_suffix('s') {
        (rest, 1_000.0)
    } else if let Some(rest) = value.strip_suffix('m') {
        (rest, 60_000.0)
    } else {
        return Err(malformed());
    };
    let parsed: f64 = number.parse().map_err(|_| malformed())?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(malformed());
    }
    let millis = parsed * multiplier;
    if millis > u64::MAX as f64 {
        return Err(malformed());
    }
    Ok(Duration::from_millis(millis.round() as u64))
}
