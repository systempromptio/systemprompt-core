use anyhow::{Result, anyhow, bail};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use systemprompt_cloud::{
    CloudApiClient, CloudPath, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_logging::CliService;

use super::select::get_credentials;
use crate::cli_settings::CliConfig;
use crate::cloud::tenant::TenantCancelArgs;
use crate::cloud::types::CancelSubscriptionOutput;
use crate::shared::CommandResult;

pub async fn cancel_subscription(
    args: TenantCancelArgs,
    config: &CliConfig,
) -> Result<CommandResult<CancelSubscriptionOutput>> {
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

    let cloud_tenants: Vec<&StoredTenant> = store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == TenantType::Cloud)
        .collect();

    if cloud_tenants.is_empty() {
        bail!("No cloud tenants found. Only cloud tenants have subscriptions.");
    }

    let tenant = if let Some(ref id) = args.id {
        store
            .tenants
            .iter()
            .find(|t| t.id == *id && t.tenant_type == TenantType::Cloud)
            .ok_or_else(|| anyhow!("Cloud tenant not found: {}", id))?
    } else {
        let options: Vec<String> = cloud_tenants
            .iter()
            .map(|t| format!("{} ({})", t.name, t.id))
            .collect();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select cloud tenant to cancel")
            .items(&options)
            .default(0)
            .interact()?;

        cloud_tenants[selection]
    };

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
    CliService::key_value("ID", &tenant.id);
    if let Some(ref hostname) = tenant.hostname {
        CliService::key_value("URL", &format!("https://{}", hostname));
    }
    CliService::info("");

    let confirmation: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Type '{}' to confirm cancellation", tenant.name))
        .interact_text()?;

    if confirmation != tenant.name {
        CliService::info("Cancellation aborted. Tenant name did not match.");
        let output = CancelSubscriptionOutput {
            tenant: tenant.id.clone(),
            tenant_name: tenant.name.clone(),
            message: "Cancellation aborted. Tenant name did not match.".to_string(),
        };
        return Ok(CommandResult::text(output)
            .with_title("Cancel Subscription")
            .with_skip_render());
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let spinner = CliService::spinner("Cancelling subscription...");
    client.cancel_subscription(&tenant.id).await?;
    spinner.finish_and_clear();

    CliService::success("Subscription cancelled");
    CliService::info("Your tenant will be suspended and all data will be destroyed.");
    CliService::info("You will not be charged for future billing periods.");
    CliService::info("");
    CliService::info(
        "Manage subscriptions: https://customer-portal.paddle.com/cpl_01j80s3z6crr7zj96htce0kr0f",
    );

    let output = CancelSubscriptionOutput {
        tenant: tenant.id.clone(),
        tenant_name: tenant.name.clone(),
        message: "Subscription cancelled. Your tenant will be suspended and all data will be \
                  destroyed."
            .to_string(),
    };

    Ok(CommandResult::text(output)
        .with_title("Cancel Subscription")
        .with_skip_render())
}
