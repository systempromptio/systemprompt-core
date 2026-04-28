use std::process::ExitCode;

use crate::auth::providers::mtls::MtlsProvider;
use crate::auth::providers::pat::PatProvider;
use crate::auth::providers::session::SessionProvider;
use crate::auth::providers::{AuthError, AuthProvider};
use crate::obs::output::{diag, emit};
use crate::{cache, config};

pub(crate) fn cmd_run() -> ExitCode {
    if let Some(cached) = cache::read_valid() {
        if emit(&cached).is_err() {
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }

    let cfg = config::load();

    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(MtlsProvider::new(&cfg)),
        Box::new(SessionProvider::new(&cfg)),
        Box::new(PatProvider::new(&cfg)),
    ];

    for provider in &chain {
        match provider.authenticate() {
            Ok(out) => {
                if let Err(e) = cache::write(&out) {
                    diag(&format!("cache write failed (continuing): {e}"));
                }
                if emit(&out).is_err() {
                    return ExitCode::from(2);
                }
                return ExitCode::SUCCESS;
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(msg)) => {
                diag(&format!("{}: {msg}", provider.name()));
                continue;
            },
        }
    }

    diag("no credential source succeeded");
    diag("run `systemprompt-cowork login <sp-live-...>` to configure a PAT");
    ExitCode::from(5)
}
