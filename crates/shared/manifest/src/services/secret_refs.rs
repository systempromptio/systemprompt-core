//! The custom-secret names the services config references: every Slack and
//! Teams app's refs, every agent's and MCP server's custom-secret grants, the
//! catalog credentials' platform secrets and the gateway's bridge-release
//! token secret. The kernel refuses these names to install members, so a core
//! member's or a spawned process's secret is never an install's read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::subprocess::SecretGrant;

use super::ServicesConfig;

fn granted(grants: &[SecretGrant]) -> impl Iterator<Item = &str> {
    grants.iter().filter_map(|grant| match grant {
        SecretGrant::Custom(name) => Some(name.as_str()),
        SecretGrant::Provider(_) | SecretGrant::EncryptionMasterKey | SecretGrant::Storage => None,
    })
}

impl ServicesConfig {
    #[must_use]
    pub fn secret_references(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for app in self.slack_apps.values() {
            names.push(app.signing_secret_ref.as_str().to_owned());
            names.push(app.bot_token_ref.as_str().to_owned());
        }
        for app in self.teams_apps.values() {
            names.push(app.app_password_ref.as_str().to_owned());
        }
        for agent in self.agents.values() {
            names.extend(granted(&agent.secrets).map(str::to_owned));
        }
        for server in self.mcp_servers.values() {
            names.extend(granted(&server.secrets).map(str::to_owned));
        }
        for credential in self.credentials.credentials.values() {
            names.extend(credential.secret.iter().map(|s| s.as_str().to_owned()));
        }
        if let Some(gateway) = self.gateway.as_ref()
            && let Some(token) = gateway
                .config()
                .bridge_releases
                .as_ref()
                .and_then(|releases| releases.token_secret.clone())
        {
            names.push(token);
        }
        names
    }

    #[must_use]
    pub fn references_secret(&self, name: &str) -> bool {
        self.secret_references()
            .iter()
            .any(|referenced| referenced.eq_ignore_ascii_case(name))
    }
}
