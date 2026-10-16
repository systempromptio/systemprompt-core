//! The rolling-update compatibility a migration declares.
//!
//! Migrations are expand-only by default: a schema they leave behind is still
//! readable by the previous release, so old pods keep serving while new ones
//! roll out. A *contract* migration — one that drops or renames something the
//! previous release reads — says so in its leading comment block:
//!
//! ```sql
//! -- @compat: breaking
//! ```
//!
//! The runner records the declaration in the database before the migration
//! runs, because a previous release cannot read a file it was never built
//! with. A binary that finds an applied migration newer than itself carrying
//! the declaration refuses to start, and `infra db migrate-plan` reports a
//! batch containing one as not rolling-safe. The build fails on any other
//! `@compat:` value, so a typo never silently downgrades a breaking change to
//! an additive one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompatDirectiveError {
    #[error("`@compat` takes only `breaking`, got `{0}`")]
    Value(String),
    #[error("`@compat` is declared {0} times; one migration states its compatibility once")]
    Repeated(usize),
}

const DIRECTIVE: &str = "-- @compat:";

pub fn parse(sql: &str) -> Result<bool, CompatDirectiveError> {
    let values: Vec<&str> = sql
        .lines()
        .map(str::trim)
        .take_while(|line| line.is_empty() || line.starts_with("--"))
        .filter_map(|line| line.strip_prefix(DIRECTIVE))
        .map(str::trim)
        .collect();
    match values.as_slice() {
        [] => Ok(false),
        ["breaking"] => Ok(true),
        [other] => Err(CompatDirectiveError::Value((*other).to_owned())),
        many => Err(CompatDirectiveError::Repeated(many.len())),
    }
}
