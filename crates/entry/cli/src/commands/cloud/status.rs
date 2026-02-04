use anyhow::Result;
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, CredentialsBootstrap};
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use crate::cli_settings::CliConfig;
use crate::cloud::types::{CloudStatusOutput, CredentialsInfo, ProfileInfo, TenantStatusInfo};
use crate::shared::CommandResult;

pub async fn execute(config: &CliConfig) -> Result<CommandResult<CloudStatusOutput>> {
    let mut tenant_id_from_profile: Option<String> = None;
    let mut profile_info: Option<ProfileInfo> = None;
    let mut credentials_info = CredentialsInfo {
        authenticated: false,
        user_email: None,
        token_expired: false,
    };
    let mut tenant_statuses: Vec<TenantStatusInfo> = Vec::new();

    match ProfileBootstrap::get() {
        Ok(profile) => {
            let mut info = ProfileInfo {
                name: profile.name.clone(),
                display_name: profile.display_name.clone(),
                tenant_id: None,
                validation_mode: None,
                credentials_path: None,
            };

            if let Some(cloud) = &profile.cloud {
                if let Ok(paths) = get_cloud_paths() {
                    let creds_path = paths.resolve(CloudPath::Credentials);
                    info.credentials_path = Some(creds_path.display().to_string());
                }
                info.validation_mode = Some(format!("{:?}", cloud.validation));

                if let Some(ref tid) = cloud.tenant_id {
                    info.tenant_id = Some(tid.clone());
                    tenant_id_from_profile = Some(tid.clone());
                }
            }
            profile_info = Some(info);
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get profile bootstrap");
        },
    }

    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            credentials_info.authenticated = true;
            credentials_info.user_email = Some(creds.user_email.clone());
            credentials_info.token_expired = creds.is_token_expired();

            let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token);

            if !config.is_json_output() {
                let spinner = CliService::spinner("Fetching tenants...");
                match api_client.list_tenants().await {
                    Ok(tenants) => {
                        spinner.finish_and_clear();
                        for tenant in &tenants {
                            let mut status_info = TenantStatusInfo {
                                id: tenant.id.clone(),
                                name: tenant.name.clone(),
                                status: "unknown".to_string(),
                                url: None,
                                message: None,
                                configured_in_profile: tenant_id_from_profile.as_deref()
                                    == Some(tenant.id.as_str()),
                            };

                            match api_client.get_tenant_status(&tenant.id).await {
                                Ok(status) => {
                                    status_info.status = status.status;
                                    status_info.url = status.app_url;
                                    status_info.message = status.message;
                                },
                                Err(e) => {
                                    status_info.status = format!("error: {}", e);
                                },
                            }
                            tenant_statuses.push(status_info);
                        }
                    },
                    Err(e) => {
                        spinner.finish_and_clear();
                        tracing::warn!(error = %e, "Could not fetch tenants");
                    },
                }
            } else {
                match api_client.list_tenants().await {
                    Ok(tenants) => {
                        for tenant in &tenants {
                            let mut status_info = TenantStatusInfo {
                                id: tenant.id.clone(),
                                name: tenant.name.clone(),
                                status: "unknown".to_string(),
                                url: None,
                                message: None,
                                configured_in_profile: tenant_id_from_profile.as_deref()
                                    == Some(tenant.id.as_str()),
                            };

                            if let Ok(status) = api_client.get_tenant_status(&tenant.id).await {
                                status_info.status = status.status;
                                status_info.url = status.app_url;
                                status_info.message = status.message;
                            }
                            tenant_statuses.push(status_info);
                        }
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, "Could not fetch tenants");
                    },
                }
            }
        },
        Ok(None) => {},
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get credentials bootstrap");
        },
    }

    let output = CloudStatusOutput {
        profile: profile_info.clone(),
        credentials: credentials_info.clone(),
        tenants: tenant_statuses.clone(),
    };

    if !config.is_json_output() {
        CliService::section("systemprompt.io Cloud Status");

        if let Some(ref profile) = profile_info {
            CliService::key_value(
                "Profile",
                &format!("{} ({})", profile.name, profile.display_name),
            );
            if let Some(ref creds_path) = profile.credentials_path {
                CliService::key_value("Credentials path", creds_path);
            }
            if let Some(ref validation_mode) = profile.validation_mode {
                CliService::key_value("Validation mode", validation_mode);
            }
            if let Some(ref tid) = profile.tenant_id {
                CliService::key_value("Tenant ID (profile)", tid);
            }
            if profile.credentials_path.is_none() && profile.validation_mode.is_none() {
                CliService::key_value("Cloud config", "Not configured");
            }
        } else {
            CliService::key_value("Profile", "Not initialized");
        }

        if credentials_info.authenticated {
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
            } else {
                for tenant in &tenant_statuses {
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
        } else {
            CliService::key_value("Authenticated", "No (not initialized)");
            CliService::info("Run 'systemprompt cloud login' to authenticate");
        }
    }

    Ok(CommandResult::card(output).with_title("Cloud Status"))
}
