use anyhow::Result;
use systemprompt_cloud::{CloudApiClient, CredentialsBootstrap};
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

pub async fn execute() -> Result<()> {
    CliService::section("systemprompt.io Cloud Status");

    let mut tenant_id_from_profile: Option<String> = None;

    match ProfileBootstrap::get() {
        Ok(profile) => {
            CliService::key_value(
                "Profile",
                &format!("{} ({})", profile.name, profile.display_name),
            );

            if let Some(cloud) = &profile.cloud {
                CliService::key_value("Credentials path", &cloud.credentials_path);
                CliService::key_value("Validation mode", &format!("{:?}", cloud.validation));

                if let Some(ref tid) = cloud.tenant_id {
                    CliService::key_value("Tenant ID (profile)", tid);
                    tenant_id_from_profile = Some(tid.clone());
                }
            } else {
                CliService::key_value("Cloud config", "Not configured");
            }
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get profile bootstrap");
            CliService::key_value("Profile", "Not initialized");
        },
    }


    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            CliService::key_value("Authenticated", "Yes");
            if let Some(email) = &creds.user_email {
                CliService::key_value("User", email);
            }
            CliService::key_value(
                "Token expired",
                if creds.is_token_expired() {
                    "Yes"
                } else {
                    "No"
                },
            );

            let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token);
            let spinner = CliService::spinner("Fetching tenants...");

            match api_client.list_tenants().await {
                Ok(tenants) => {
                    spinner.finish_and_clear();
                    if tenants.is_empty() {
                        CliService::info("No tenants found for this account");
                    } else {
                        for tenant in &tenants {
                            CliService::section(&format!(
                                "Tenant: {} ({})",
                                tenant.name, tenant.id
                            ));

                            match api_client.get_tenant_status(&tenant.id).await {
                                Ok(status) => {
                                    CliService::key_value("Status", &status.status);
                                    if let Some(url) = &status.app_url {
                                        CliService::key_value("URL", url);
                                    }
                                    if let Some(msg) = &status.message {
                                        CliService::info(&format!("Message: {}", msg));
                                    }
                                },
                                Err(e) => {
                                    CliService::warning(&format!("Could not fetch status: {}", e));
                                },
                            }

                            if tenant_id_from_profile.as_deref() == Some(tenant.id.as_str()) {
                                CliService::info("(configured in profile)");
                            }
                        }
                    }
                },
                Err(e) => {
                    spinner.finish_and_clear();
                    CliService::warning(&format!("Could not fetch tenants: {}", e));
                },
            }
        },
        Ok(None) => {
            CliService::key_value("Authenticated", "No (cloud disabled or not configured)");
        },
        Err(e) => {
            tracing::debug!(error = %e, "Failed to get credentials bootstrap");
            CliService::key_value("Authenticated", "No (not initialized)");
            CliService::info("Run 'systemprompt cloud login' to authenticate");
        },
    }

    Ok(())
}
