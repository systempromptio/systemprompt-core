//! CLI rendering for the tenant provisioning flow.
//!
//! Implements the cloud crate's [`ProvisioningProgress`] seam over
//! `CliService` spinners and message sinks; the
//! [`TenantProvisioningService`](systemprompt_cloud::tenants::TenantProvisioningService)
//! owns sequencing, this type owns presentation.

use std::sync::Mutex;

use indicatif::ProgressBar;
use systemprompt_cloud::tenants::{ProvisioningProgress, ProvisioningProgressEvent};
use systemprompt_logging::CliService;
use url::Url;

pub(super) struct CliProvisioningProgress {
    spinner: Mutex<Option<ProgressBar>>,
}

impl CliProvisioningProgress {
    pub(super) const fn new() -> Self {
        Self {
            spinner: Mutex::new(None),
        }
    }

    fn start_spinner(&self, message: &str) {
        if let Ok(mut slot) = self.spinner.lock() {
            *slot = Some(CliService::spinner(message));
        }
    }

    fn clear_spinner(&self) {
        if let Ok(mut slot) = self.spinner.lock()
            && let Some(spinner) = slot.take()
        {
            spinner.finish_and_clear();
        }
    }
}

impl ProvisioningProgress for CliProvisioningProgress {
    fn event(&self, event: &ProvisioningProgressEvent<'_>) {
        if !matches!(event, ProvisioningProgressEvent::ProvisioningUpdate { .. }) {
            self.clear_spinner();
        }
        match event {
            ProvisioningProgressEvent::CheckoutSessionStarted => {
                self.start_spinner("Creating checkout session...");
            },
            ProvisioningProgressEvent::CheckoutComplete { tenant_id } => {
                CliService::success(&format!(
                    "Checkout complete! Tenant ID: {}",
                    tenant_id.as_str()
                ));
            },
            ProvisioningProgressEvent::ProvisioningStarted => {
                self.start_spinner("Waiting for infrastructure provisioning...");
            },
            ProvisioningProgressEvent::ProvisioningUpdate { message } => {
                CliService::info(message);
            },
            ProvisioningProgressEvent::Provisioned => {
                CliService::success("Tenant provisioned successfully");
            },
            ProvisioningProgressEvent::CredentialsFetchStarted => {
                self.start_spinner("Fetching database credentials...");
            },
            ProvisioningProgressEvent::CredentialsFetched => {
                CliService::success("Database credentials retrieved");
            },
            ProvisioningProgressEvent::ExternalAccessStarted => {
                self.start_spinner("Enabling external database access...");
            },
            ProvisioningProgressEvent::ExternalAccessEnabled { database_url } => {
                CliService::success("External database access enabled");
                print_database_connection_info(database_url);
            },
            ProvisioningProgressEvent::ExternalAccessFailed { error } => {
                CliService::warning(&format!("Failed to enable external access: {}", error));
                CliService::info("You can enable it later with 'systemprompt cloud tenant edit'");
            },
            ProvisioningProgressEvent::TenantSyncStarted => {
                self.start_spinner("Syncing new tenant...");
            },
            ProvisioningProgressEvent::CheckoutSessionCreated
            | ProvisioningProgressEvent::TenantSynced => {},
        }
    }
}

fn print_database_connection_info(url: &str) {
    let Ok(parsed) = Url::parse(url) else {
        return;
    };

    let host = parsed.host_str().unwrap_or("unknown");
    let port = parsed.port().unwrap_or(5432);
    let database = parsed.path().trim_start_matches('/');
    let username = parsed.username();
    let password = parsed.password().unwrap_or("********");

    CliService::section("Database Connection");
    CliService::key_value("Host", host);
    CliService::key_value("Port", &port.to_string());
    CliService::key_value("Database", database);
    CliService::key_value("User", username);
    CliService::key_value("Password", password);
    CliService::key_value("SSL", "required");
    CliService::info("");
    CliService::key_value("Connection URL", url);
    CliService::info("");
    CliService::info(&format!(
        "Connect with psql:\n  PGPASSWORD='{}' psql -h {} -p {} -U {} -d {}",
        password, host, port, username, database
    ));
}
