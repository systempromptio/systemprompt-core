use std::process::ExitCode;

use crate::auth::providers::AuthError;
use crate::auth::secret::Secret;
use crate::auth::{cache, provider_chain};
use crate::config;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;

fn acquire_bearer() -> Option<Secret> {
    if let Some(out) = cache::read_valid() {
        return Some(out.token);
    }
    let cfg = config::load();
    let chain = provider_chain(&cfg);
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = cache::write(&out);
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => {},
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
    let Some(bearer) = acquire_bearer() else {
        diag("no credential available; run `systemprompt-cowork login` first");
        return ExitCode::from(5);
    };

    let client = GatewayClient::new(gateway.clone());
    match client.fetch_whoami(bearer.expose()) {
        Ok(value) => {
            match serde_json::to_string_pretty(&value) {
                Ok(s) => println!("{s}"),
                Err(_) => println!("{value:?}"),
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("whoami failed: {e}"));
            ExitCode::from(3)
        },
    }
}
