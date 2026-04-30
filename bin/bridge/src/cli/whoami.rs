use std::process::ExitCode;

use crate::auth::ChainError;
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;
use crate::{auth, config};

pub(crate) fn cmd_whoami() -> ExitCode {
    match crate::proxy::block_on(async {
        let cfg = config::load();
        let gateway = config::gateway_url_or_default(&cfg);
        let out = match auth::acquire_bearer(&cfg).await {
            Ok(out) => out,
            Err(ChainError::PreferredTransient { provider, source }) => {
                diag(&format!(
                    "transient auth failure on preferred provider {provider}: {source}"
                ));
                return ExitCode::from(10);
            },
            Err(ChainError::NoneSucceeded) => {
                diag("no credential available; run `systemprompt-bridge login` first");
                return ExitCode::from(5);
            },
        };

        let client = GatewayClient::new(gateway.clone());
        match client.fetch_whoami(out.token.expose()).await {
            Ok(value) => {
                match serde_json::to_string_pretty(&value) {
                    Ok(s) => output::print_line(&s),
                    Err(_) => output::print_line(&format!("{value:?}")),
                }
                ExitCode::SUCCESS
            },
            Err(e) => {
                diag(&format!("whoami failed: {e}"));
                ExitCode::from(3)
            },
        }
    }) {
        Ok(code) => code,
        Err(e) => {
            diag(&format!("runtime init failed: {e}"));
            ExitCode::from(70)
        },
    }
}
