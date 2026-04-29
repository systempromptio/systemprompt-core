use std::process::ExitCode;

use crate::gateway::GatewayClient;
use crate::obs::output::diag;
use crate::{auth, config};

pub(crate) fn cmd_whoami() -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let Some(out) = auth::acquire_bearer(&cfg) else {
        diag("no credential available; run `systemprompt-cowork login` first");
        return ExitCode::from(5);
    };

    let client = GatewayClient::new(gateway.clone());
    match client.fetch_whoami(out.token.expose()) {
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
