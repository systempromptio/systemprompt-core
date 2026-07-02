//! Cloud tenant creation via subscription checkout.
//!
//! Validates a release build, prompts for the plan, region, and external
//! database access, then hands a [`TenantCreatePlan`] to the cloud crate's
//! [`TenantProvisioningService`], which drives the Paddle checkout callback,
//! provisioning wait, and credential retrieval. Afterwards a profile is
//! written for the new tenant and, when required, the initial deploy runs
//! through the sync crate's [`DeployOrchestrator`].

use anyhow::{Context, Result, anyhow, bail};
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::tenants::{TenantCreatePlan, TenantProvisioningService};
use systemprompt_cloud::{
    CheckoutTemplates, CloudApiClient, CloudCredentials, ProfilePath, ProjectContext, StoredTenant,
};
use systemprompt_identifiers::{PriceId, TenantId};
use systemprompt_logging::CliService;
use systemprompt_sync::deploy::{DeployOptions, DeployOrchestrator, DeployRequest};

use crate::cloud::deploy::CliDeployProgress;
use crate::cloud::profile::{collect_api_keys, create_profile_for_tenant};
use crate::cloud::templates::{CHECKOUT_ERROR_HTML, CHECKOUT_SUCCESS_HTML, WAITING_HTML};
use crate::interactive::Prompter;
use crate::shared::project::ProjectRoot;

use super::super::validation::{validate_build_ready, warn_required_secrets};
use super::progress::CliProvisioningProgress;

pub async fn create_cloud_tenant(
    creds: &CloudCredentials,
    _default_region: &str,
    prompter: &dyn Prompter,
) -> Result<StoredTenant> {
    let validation = validate_build_ready().context(
        "Cloud tenant creation requires a built project.\nRun 'just build --release' before \
         creating a cloud tenant.",
    )?;

    CliService::success("Build validation passed");
    CliService::info("Creating cloud tenant via subscription");

    let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;

    let plan = assemble_plan(&client, prompter).await?;

    let progress = CliProvisioningProgress::new();
    let provisioned = TenantProvisioningService::new(&client)
        .provision(&plan, &progress)
        .await?;
    let stored_tenant = provisioned.tenant;

    CliService::section("Profile Setup");
    let profile_name = prompter.input_with_default("Profile name", &stored_tenant.name)?;

    CliService::section("API Keys");
    let api_keys = collect_api_keys(prompter)?;

    let profile = create_profile_for_tenant(
        prompter,
        &stored_tenant,
        &api_keys,
        &profile_name,
        Some(&creds.api_url),
    )?;
    CliService::success(&format!("Profile '{}' created", profile.name));

    if provisioned.needs_deploy {
        CliService::section("Initial Deploy");
        CliService::info("Deploying your code with profile configuration...");
        run_initial_deploy(creds, &stored_tenant, &profile.name).await?;
    }

    warn_required_secrets(&validation.required_secrets);

    Ok(stored_tenant)
}

async fn assemble_plan(
    client: &CloudApiClient,
    prompter: &dyn Prompter,
) -> Result<TenantCreatePlan> {
    let price_id = select_plan(client, prompter).await?;
    let region = select_region(prompter)?;
    let external_db_access = prompt_external_access(prompter)?;

    Ok(TenantCreatePlan {
        price_id,
        region: region.to_owned(),
        redirect_uri: format!("http://127.0.0.1:{}/callback", CALLBACK_PORT),
        external_db_access,
        templates: CheckoutTemplates {
            success_html: CHECKOUT_SUCCESS_HTML,
            error_html: CHECKOUT_ERROR_HTML,
            waiting_html: WAITING_HTML,
        },
    })
}

async fn select_plan(client: &CloudApiClient, prompter: &dyn Prompter) -> Result<PriceId> {
    let spinner = CliService::spinner("Fetching available plans...");
    let plans = client.get_plans().await?;
    spinner.finish_and_clear();

    if plans.is_empty() {
        bail!("No plans available. Please contact support.");
    }

    let plan_options: Vec<String> = plans.iter().map(|p| p.name.clone()).collect();

    let plan_selection = prompter.select("Select a plan", &plan_options)?;

    Ok(plans[plan_selection].paddle_price_id.clone())
}

fn select_region(prompter: &dyn Prompter) -> Result<&'static str> {
    let region_options: Vec<String> = AVAILABLE
        .iter()
        .map(|(code, name)| format!("{} ({})", name, code))
        .collect();

    let region_selection = prompter.select("Select a region", &region_options)?;

    Ok(AVAILABLE[region_selection].0)
}

fn prompt_external_access(prompter: &dyn Prompter) -> Result<bool> {
    CliService::section("Database Access");
    CliService::info(
        "External database access allows direct PostgreSQL connections from your local machine.",
    );
    CliService::info("This is required for local development workflows.");

    let enable_external = prompter.confirm("Enable external database access?", true)?;

    if !enable_external {
        CliService::info("External access disabled. Some local features will be limited.");
    }

    Ok(enable_external)
}

async fn run_initial_deploy(
    creds: &CloudCredentials,
    tenant: &StoredTenant,
    profile_name: &str,
) -> Result<()> {
    let project = ProjectRoot::discover().map_err(|e| anyhow!("{}", e))?;
    let ctx = ProjectContext::discover();
    let profile_dir = ctx.profile_dir(profile_name);

    let request = DeployRequest {
        tenant_id: TenantId::new(tenant.id.clone()),
        tenant_name: tenant.name.clone(),
        profile_name: profile_name.to_owned(),
        project_root: project.as_path().to_path_buf(),
        credentials: creds.clone(),
        hostname: tenant.hostname.clone(),
        secrets_path: ProfilePath::Secrets.resolve(&profile_dir),
        signing_key_path: profile_dir.join("signing_key.pem"),
        options: DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: None,
        },
    };

    let progress = CliDeployProgress::non_interactive();
    DeployOrchestrator::new()
        .deploy(&request, &progress)
        .await?;

    Ok(())
}
