use anyhow::{bail, Result};
use clap::Subcommand;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::CloudApiClient;
use systemprompt_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::tenant::get_credentials;
use crate::cli_settings::CliConfig;

#[derive(Debug, Subcommand)]
pub enum DomainCommands {
    #[command(about = "Set custom domain for tenant")]
    Set {
        #[arg(help = "Domain name (e.g., example.com)")]
        domain: String,
    },

    #[command(about = "Check custom domain status")]
    Status,

    #[command(about = "Remove custom domain")]
    Remove {
        #[arg(short = 'y', long, help = "Skip confirmation")]
        yes: bool,
    },
}

pub async fn execute(cmd: DomainCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        DomainCommands::Set { domain } => set_domain(domain).await,
        DomainCommands::Status => get_status().await,
        DomainCommands::Remove { yes } => remove_domain(yes, config).await,
    }
}

fn get_tenant_id() -> Result<String> {
    let profile =
        ProfileBootstrap::get().map_err(|_| anyhow::anyhow!("Profile not initialized"))?;

    let cloud = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cloud not configured in profile"))?;

    cloud
        .tenant_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("No tenant_id in profile. Create a cloud tenant first."))
}

async fn set_domain(domain: String) -> Result<()> {
    CliService::section("Set Custom Domain");

    let tenant_id = get_tenant_id()?;
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let spinner = CliService::spinner(&format!("Configuring domain {}...", domain));
    match client.set_custom_domain(&tenant_id, &domain).await {
        Ok(response) => {
            spinner.finish_and_clear();

            CliService::success("Custom Domain Configuration");
            CliService::info("");
            CliService::info(&format!("  Domain:      {}", response.domain));
            CliService::info(&format!("  Status:      {}", response.status));
            CliService::info(&format!("  DNS Target:  {}", response.dns_target));
            CliService::info("");

            CliService::info("DNS Configuration Required:");
            CliService::info(&format!(
                "    Type:   {}",
                response.dns_instructions.record_type
            ));
            CliService::info(&format!("    Host:   {}", response.dns_instructions.host));
            CliService::info(&format!("    Value:  {}", response.dns_instructions.value));
            CliService::info(&format!("    TTL:    {}", response.dns_instructions.ttl));
            CliService::info("");

            CliService::info(
                "After configuring DNS, run 'systemprompt cloud domain status' to verify.",
            );
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to set custom domain: {}", e);
        },
    }

    Ok(())
}

async fn get_status() -> Result<()> {
    CliService::section("Custom Domain Status");

    let tenant_id = get_tenant_id()?;
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let spinner = CliService::spinner("Checking domain status...");
    match client.get_custom_domain(&tenant_id).await {
        Ok(response) => {
            spinner.finish_and_clear();

            CliService::info("");
            CliService::info(&format!("  Domain:      {}", response.domain));
            CliService::info(&format!("  Status:      {}", response.status));
            CliService::info(&format!(
                "  Verified:    {}",
                if response.verified { "Yes" } else { "No" }
            ));
            CliService::info(&format!("  DNS Target:  {}", response.dns_target));

            if let Some(created) = &response.created_at {
                CliService::info(&format!("  Created:     {}", created));
            }
            if let Some(verified) = &response.verified_at {
                CliService::info(&format!("  Verified:    {}", verified));
            }
            CliService::info("");

            if !response.verified {
                CliService::info("DNS Configuration Required:");
                CliService::info(&format!(
                    "    Type:   {}",
                    response.dns_instructions.record_type
                ));
                CliService::info(&format!("    Host:   {}", response.dns_instructions.host));
                CliService::info(&format!("    Value:  {}", response.dns_instructions.value));
                CliService::info(&format!("    TTL:    {}", response.dns_instructions.ttl));
                CliService::info("");
            }
        },
        Err(e) => {
            spinner.finish_and_clear();
            let err_str = e.to_string();
            if err_str.contains("not_found") || err_str.contains("404") {
                CliService::info("No custom domain configured for this tenant.");
                CliService::info("Use 'systemprompt cloud domain set <domain>' to configure one.");
                return Ok(());
            }
            bail!("Failed to get domain status: {}", e);
        },
    }

    Ok(())
}

async fn remove_domain(yes: bool, config: &CliConfig) -> Result<()> {
    CliService::section("Remove Custom Domain");

    let tenant_id = get_tenant_id()?;
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let domain_name = match client.get_custom_domain(&tenant_id).await {
        Ok(response) => response.domain,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("not_found") || err_str.contains("404") {
                CliService::info("No custom domain configured for this tenant.");
                return Ok(());
            }
            bail!("Failed to get domain status: {}", e);
        },
    };

    if !yes {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "--yes is required in non-interactive mode for domain removal"
            ));
        }

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Remove custom domain '{}'? This will delete the TLS certificate.",
                domain_name
            ))
            .default(false)
            .interact()?;

        if !confirm {
            CliService::info("Cancelled");
            return Ok(());
        }
    }

    let spinner = CliService::spinner(&format!("Removing domain {}...", domain_name));
    match client.delete_custom_domain(&tenant_id).await {
        Ok(()) => {
            spinner.finish_and_clear();
            CliService::success(&format!(
                "Custom domain '{}' removed successfully",
                domain_name
            ));
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to remove custom domain: {}", e);
        },
    }

    Ok(())
}
