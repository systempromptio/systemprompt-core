use anyhow::{Result, anyhow, bail};
use systemprompt_cloud::{
    CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_logging::CliService;

use super::select::get_credentials;
use crate::cli_settings::CliConfig;
use crate::cloud::tenant::TenantCancelArgs;
use crate::cloud::types::CancelSubscriptionOutput;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

pub async fn cancel_subscription(
    args: TenantCancelArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_interactive() {
        bail!(
            "Subscription cancellation requires interactive mode for safety.\nThis is an \
             irreversible operation that destroys all data."
        );
    }

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store =
        TenantStore::load_from_path(&tenants_path).unwrap_or_else(|_| TenantStore::default());

    let tenant = select_cancellation_target(&store, prompter, args.id.as_ref())?;

    render_cancellation_warning(tenant);

    let confirmation =
        prompter.input(&format!("Type '{}' to confirm cancellation", tenant.name))?;

    if confirmation != tenant.name {
        CliService::info("Cancellation aborted. Tenant name did not match.");
        let output = CancelSubscriptionOutput {
            tenant: tenant.id.as_str().to_owned(),
            tenant_name: tenant.name.clone(),
            message: "Cancellation aborted. Tenant name did not match.".to_owned(),
        };
        return Ok(CommandOutput::card_value("Cancel Subscription", &output).with_skip_render());
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;

    let spinner = CliService::spinner("Cancelling subscription...");
    client.cancel_subscription(&tenant.id).await?;
    spinner.finish_and_clear();

    render_cancellation_complete();

    let output = CancelSubscriptionOutput {
        tenant: tenant.id.as_str().to_owned(),
        tenant_name: tenant.name.clone(),
        message: "Subscription cancelled. Your tenant will be suspended and all data will be \
                  destroyed."
            .to_owned(),
    };

    Ok(CommandOutput::card_value("Cancel Subscription", &output).with_skip_render())
}

fn select_cancellation_target<'a>(
    store: &'a TenantStore,
    prompter: &dyn Prompter,
    id: Option<&String>,
) -> Result<&'a StoredTenant> {
    let cloud_tenants: Vec<&StoredTenant> = store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == TenantType::Cloud)
        .collect();

    if cloud_tenants.is_empty() {
        bail!("No cloud tenants found. Only cloud tenants have subscriptions.");
    }

    if let Some(id) = id {
        store
            .tenants
            .iter()
            .find(|t| t.id.as_str() == id.as_str() && t.tenant_type == TenantType::Cloud)
            .ok_or_else(|| anyhow!("Cloud tenant not found: {}", id))
    } else {
        let options: Vec<String> = cloud_tenants
            .iter()
            .map(|t| format!("{} ({})", t.name, t.id))
            .collect();

        let selection = prompter.select("Select cloud tenant to cancel", &options)?;

        Ok(cloud_tenants[selection])
    }
}

fn render_cancellation_warning(tenant: &StoredTenant) {
    CliService::section("⚠️  CANCEL SUBSCRIPTION");
    CliService::error("THIS ACTION IS IRREVERSIBLE");
    CliService::info("");
    CliService::info("This will:");
    CliService::info("  • Cancel your subscription immediately");
    CliService::info("  • Stop and destroy the Fly.io machine");
    CliService::info("  • Delete ALL data in the database");
    CliService::info("  • Remove all deployed code and configuration");
    CliService::info("");
    CliService::warning("Your data CANNOT be recovered after this operation.");
    CliService::info("");

    CliService::key_value("Tenant", &tenant.name);
    CliService::key_value("ID", tenant.id.as_str());
    if let Some(ref hostname) = tenant.hostname {
        CliService::key_value("URL", &format!("https://{}", hostname));
    }
    CliService::info("");
}

fn render_cancellation_complete() {
    CliService::success("Subscription cancelled");
    CliService::info("Your tenant will be suspended and all data will be destroyed.");
    CliService::info("You will not be charged for future billing periods.");
    CliService::info("");
    CliService::info(
        "Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f",
    );
}
