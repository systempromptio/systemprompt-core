use std::process::ExitCode;

use crate::auth::ChainError;
use crate::obs::output::{diag, emit};
use crate::{auth, config};

pub(crate) fn cmd_run() -> ExitCode {
    let cfg = config::load();
    let acquired = match crate::proxy::block_on(auth::acquire_bearer(&cfg)) {
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
            diag("run `systemprompt-bridge login <sp-live-...>` to configure a PAT");
            return ExitCode::from(5);
        },
    };
    if emit(&out).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
