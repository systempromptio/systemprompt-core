use std::process::ExitCode;

use crate::obs::output::{diag, emit};
use crate::{auth, config};

pub(crate) fn cmd_run() -> ExitCode {
    let cfg = config::load();
    let Some(out) = auth::acquire_bearer(&cfg) else {
        diag("no credential source succeeded");
        diag("run `systemprompt-cowork login <sp-live-...>` to configure a PAT");
        return ExitCode::from(5);
    };
    if emit(&out).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
