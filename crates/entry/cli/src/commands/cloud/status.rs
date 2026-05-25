use anyhow::Result;
use systemprompt_cloud::{CloudApiClient, CloudPath, CredentialsBootstrap, get_cloud_paths};
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::cloud::types::{CloudStatusOutput, CredentialsInfo, ProfileInfo, TenantStatusInfo};
use crate::shared::CommandResult;

pub(super) async fn execute(config: &CliConfig) -> Result<CommandResult<CloudStatusOutput>> {
    let (profile_info, tenant_id_from_profile) = load_profile_info();
    let (credentials_info, tenant_statuses) =
        load_credentials_and_tenants(config, tenant_id_from_profile.as_deref()).await?;

    let output = CloudStatusOutput {
        profile: profile_info.clone(),
        credentials: credentials_info.clone(),
        tenants: tenant_statuses.clone(),
    };

    if !config.is_json_output() {
        render_status(profile_info.as_ref(), &credentials_info, &tenant_statuses);
    }

    Ok(CommandResult::card(output).with_title("Cloud Status"))
}

fn load_profile_info() -> (Option<ProfileInfo>, Option<String>) {
    let profile = match ProfileBootstrap::get() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get profile bootstrap");
            return (None, None);
        },
    };

    let mut info = ProfileInfo {
        name: profile.name.clone(),
        display_name: Some(profile.display_name.clone()),
        database_url: None,
        tenant_id: None,
        validation_mode: None,
        credentials_path: None,
        routing: None,
        is_active: None,
        session_status: None,
    };
    let mut tenant_id_from_profile = None;

    if let Some(cloud) = &profile.cloud {
        let paths = get_cloud_paths();
        let creds_path = paths.resolve(CloudPath::Credentials);
        info.credentials_path = Some(creds_path.display().to_string());
        info.validation_mode = Some(format!("{:?}", cloud.validation));
        if let Some(ref tid) = cloud.tenant_id {
            info.tenant_id = Some(tid.clone());
            tenant_id_from_profile = Some(tid.as_str().to_owned());
        }
    }

    (Some(info), tenant_id_from_profile)
}

async fn load_credentials_and_tenants(
    config: &CliConfig,
    tenant_id_from_profile: Option<&str>,
) -> Result<(CredentialsInfo, Vec<TenantStatusInfo>)> {
    let mut credentials_info = CredentialsInfo {
        authenticated: false,
        user_email: None,
        token_expired: false,
    };
    let mut tenant_statuses = Vec::new();

    let creds = match CredentialsBootstrap::get() {
        Ok(Some(c)) => c,
        Ok(None) => return Ok((credentials_info, tenant_statuses)),
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get credentials bootstrap");
            return Ok((credentials_info, tenant_statuses));
        },
    };

    credentials_info.authenticated = true;
    credentials_info.user_email = Some(creds.user_email.clone());
    credentials_info.token_expired = creds.is_token_expired();

    let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;
    let spinner = (!config.is_json_output()).then(|| CliService::spinner("Fetching tenants..."));
    let json_mode = config.is_json_output();
    let tenants_result = api_client.list_tenants().await;
    if let Some(s) = &spinner {
        s.finish_and_clear();
    }
    let tenants = match tenants_result {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Could not fetch tenants");
            return Ok((credentials_info, tenant_statuses));
        },
    };

    for tenant in &tenants {
        let mut status_info = TenantStatusInfo {
            id: tenant.id.clone(),
            name: tenant.name.clone(),
            status: "unknown".to_owned(),
            url: None,
            message: None,
            configured_in_profile: tenant_id_from_profile == Some(tenant.id.as_str()),
        };
        match api_client
            .get_tenant_status(&systemprompt_identifiers::TenantId::new(&tenant.id))
            .await
        {
            Ok(status) => {
                status_info.status = status.status;
                status_info.url = status.app_url;
                status_info.message = status.message;
            },
            Err(e) if !json_mode => {
                status_info.status = format!("error: {}", e);
            },
            Err(_) => {},
        }
        tenant_statuses.push(status_info);
    }

    Ok((credentials_info, tenant_statuses))
}

fn render_status(
    profile_info: Option<&ProfileInfo>,
    credentials_info: &CredentialsInfo,
    tenant_statuses: &[TenantStatusInfo],
) {
    CliService::section("systemprompt.io Cloud Status");
    render_profile(profile_info);
    render_credentials(credentials_info, tenant_statuses);
}

fn render_profile(profile_info: Option<&ProfileInfo>) {
    let Some(profile) = profile_info else {
        CliService::key_value("Profile", "Not initialized");
        return;
    };
    CliService::key_value(
        "Profile",
        &format!(
            "{} ({})",
            profile.name,
            profile.display_name.as_deref().unwrap_or("")
        ),
    );
    if let Some(ref creds_path) = profile.credentials_path {
        CliService::key_value("Credentials path", creds_path);
    }
    if let Some(ref validation_mode) = profile.validation_mode {
        CliService::key_value("Validation mode", validation_mode);
    }
    if let Some(ref tid) = profile.tenant_id {
        CliService::key_value("Tenant ID (profile)", tid.as_str());
    }
    if profile.credentials_path.is_none() && profile.validation_mode.is_none() {
        CliService::key_value("Cloud config", "Not configured");
    }
}

fn render_credentials(credentials_info: &CredentialsInfo, tenant_statuses: &[TenantStatusInfo]) {
    if !credentials_info.authenticated {
        CliService::key_value("Authenticated", "No (not initialized)");
        CliService::info("Run 'systemprompt cloud login' to authenticate");
        return;
    }
    CliService::key_value("Authenticated", "Yes");
    if let Some(ref email) = credentials_info.user_email {
        CliService::key_value("User", email);
    }
    CliService::key_value(
        "Token expired",
        if credentials_info.token_expired {
            "Yes"
        } else {
            "No"
        },
    );
    if tenant_statuses.is_empty() {
        CliService::info("No tenants found for this account");
        return;
    }
    for tenant in tenant_statuses {
        CliService::section(&format!("Tenant: {} ({})", tenant.name, tenant.id));
        CliService::key_value("Status", &tenant.status);
        if let Some(ref url) = tenant.url {
            CliService::key_value("URL", url);
        }
        if let Some(ref msg) = tenant.message {
            CliService::info(&format!("Message: {}", msg));
        }
        if tenant.configured_in_profile {
            CliService::info("(configured in profile)");
        }
    }
}
