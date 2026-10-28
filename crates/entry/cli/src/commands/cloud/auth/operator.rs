//! Operator sign-in against an OIDC issuer for self-managed tenants.
//!
//! Self-managed tenants trust the operator's own identity provider, not the
//! systemprompt.io control plane. The sign-in stores the issuer's ID token;
//! remote CLI routing and `cloud fleet` exchange it per tenant for a tenant
//! session (RFC 8693) at the tenant's token endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use systemprompt_cloud::operator::{OperatorLoginRequest, run_operator_login};
use systemprompt_cloud::{CloudPath, OAuthTemplates, get_cloud_paths};
use systemprompt_identifiers::ClientId;
use systemprompt_logging::CliService;
use systemprompt_manifest::{OperatorAuthConfig, Profile, default_operator_scopes};

use crate::cli_settings::CliConfig;
use crate::cloud::templates::{AUTH_ERROR_HTML, AUTH_SUCCESS_HTML};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Serialize)]
struct OperatorLoginOutput {
    issuer: String,
    client_id: ClientId,
    email: Option<String>,
    credentials_path: String,
}

pub(super) fn operator_request(
    issuer: Option<String>,
    client_id: Option<ClientId>,
    profile: Option<&Profile>,
) -> Result<Option<OperatorLoginRequest>> {
    let configured: Option<&OperatorAuthConfig> = profile
        .and_then(|p| p.cloud.as_ref())
        .filter(|cloud| cloud.provider.is_self_managed())
        .and_then(|cloud| cloud.operator.as_ref());
    match (issuer, client_id) {
        (Some(issuer), Some(client_id)) => Ok(Some(OperatorLoginRequest {
            issuer,
            client_id,
            scopes: configured.map_or_else(default_operator_scopes, |c| c.scopes.clone()),
        })),
        (Some(_), None) | (None, Some(_)) => {
            bail!("--issuer and --client-id must be given together")
        },
        (None, None) => Ok(configured.map(|c| OperatorLoginRequest {
            issuer: c.issuer.clone(),
            client_id: c.client_id.clone(),
            scopes: c.scopes.clone(),
        })),
    }
}

pub(super) async fn execute(
    request: &OperatorLoginRequest,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_interactive() {
        return Err(anyhow!("Operator sign-in requires interactive mode."));
    }
    CliService::section("Operator Sign-in");
    CliService::key_value("Issuer", &request.issuer);

    let templates = OAuthTemplates {
        success_html: AUTH_SUCCESS_HTML,
        error_html: AUTH_ERROR_HTML,
    };
    let credentials = run_operator_login(request, templates).await?;

    let path = get_cloud_paths().resolve(CloudPath::OperatorCredentials);
    credentials.save_to_path(&path)?;
    CliService::success("Signed in");
    CliService::key_value("Credentials saved to", &path.display().to_string());

    let output = OperatorLoginOutput {
        issuer: credentials.issuer.clone(),
        client_id: credentials.client_id.clone(),
        email: credentials.email.as_ref().map(|e| e.as_str().to_owned()),
        credentials_path: path.display().to_string(),
    };
    Ok(CommandOutput::card_value("Operator Sign-in", &output).with_skip_render())
}
