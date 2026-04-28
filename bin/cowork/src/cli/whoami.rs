use std::process::ExitCode;

use crate::auth::providers::mtls::MtlsProvider;
use crate::auth::providers::pat::PatProvider;
use crate::auth::providers::session::SessionProvider;
use crate::auth::providers::{AuthError, AuthProvider};
use crate::cache;
use crate::config;
use crate::http;
use crate::obs::output::diag;
use crate::secret::Secret;

fn acquire_bearer() -> Option<Secret> {
    if let Some(out) = cache::read_valid() {
        return Some(out.token);
    }
    let cfg = config::load();
    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(MtlsProvider::new(&cfg)),
        Box::new(SessionProvider::new(&cfg)),
        Box::new(PatProvider::new(&cfg)),
    ];
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = cache::write(&out);
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(msg)) => {
                diag(&format!("{}: {msg}", p.name()));
            },
        }
    }
    None
}

pub(crate) fn cmd_whoami() -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let bearer = match acquire_bearer() {
        Some(t) => t,
        None => {
            diag("no credential available; run `systemprompt-cowork login` first");
            return ExitCode::from(5);
        },
    };

    let client = http::GatewayClient::new(gateway);
    match client.fetch_whoami(bearer.expose()) {
        Ok(value) => {
            match serde_json::to_string_pretty(&value) {
                Ok(s) => println!("{s}"),
                Err(_) => println!("{value}"),
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("whoami failed: {e}"));
            ExitCode::from(3)
        },
    }
}
