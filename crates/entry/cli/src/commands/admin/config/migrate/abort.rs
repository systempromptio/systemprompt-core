//! Why `config migrate` stopped: a fact of the legacy tree it cannot decide.
//!
//! Every abort names the file and the keys involved and what the operator
//! must decide; migrate never guesses and never writes after an abort.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
#[error("{file}: {reason} [{keys}]; nothing was written")]
pub struct MigrateAbort {
    pub file: String,
    pub keys: String,
    pub reason: String,
}

impl MigrateAbort {
    pub fn new(
        file: &str,
        keys: impl IntoIterator<Item = impl AsRef<str>>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            file: file.to_owned(),
            keys: keys
                .into_iter()
                .map(|k| k.as_ref().to_owned())
                .collect::<Vec<_>>()
                .join(", "),
            reason: reason.into(),
        }
    }
}

pub type MigrateResult<T> = Result<T, MigrateAbort>;

pub const PROVIDERS_FILE: &str = "ai/providers.yaml";
pub const GATEWAY_FILE: &str = "ai/gateway.yaml";
pub const AI_FILE: &str = "ai/config.yaml";
