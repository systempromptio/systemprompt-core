//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::obs::output::{diag, emit};
use crate::{auth, config};

pub(super) fn cmd_run() -> ExitCode {
    let cfg = config::load();
    let session_id = SessionId::generate();
    let acquired = match crate::proxy::block_on(auth::acquire_bearer(&cfg, &session_id)) {
        Ok(r) => r,
        Err(e) => {
            diag(&format!("runtime init failed: {e}"));
            return ExitCode::from(70);
        },
    };
    let out = match acquired {
        Ok(out) => out,
        Err(ChainError::PreferredTransient { provider, source }) => {
            diag(&format!(
                "transient auth failure on preferred provider {provider}: {source}"
            ));
            return ExitCode::from(10);
        },
        Err(ChainError::NoneSucceeded) => {
            diag("no credential source succeeded");
            diag(&format!(
                "run `{} login <sp-live-...>` to configure a PAT",
                crate::brand::brand().binary_name
            ));
            return ExitCode::from(5);
        },
    };
    if emit(&out).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
