pub mod cache;
pub mod config;
pub mod http;
pub mod keystore;
pub mod loopback;
pub mod output;
pub mod providers;
pub mod types;

use std::process::ExitCode;

use crate::output::{diag, emit};
use crate::providers::mtls::MtlsProvider;
use crate::providers::pat::PatProvider;
use crate::providers::session::SessionProvider;
use crate::providers::{AuthError, AuthProvider};

pub fn run() -> ExitCode {
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
    diag("run setup: open http://localhost:3000/cowork-auth/setup");
    ExitCode::from(5)
}
