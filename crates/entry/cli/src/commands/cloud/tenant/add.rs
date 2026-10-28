//! `cloud tenant add` command: record a self-managed tenant.
//!
//! Writes a Kubernetes, GKE or local Docker tenant into the tenant store
//! without the systemprompt.io control plane. The record is never touched by
//! control-plane sync; `cloud tenant delete` forgets it again without touching
//! the deployment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use clap::Args;
use systemprompt_cloud::{
    CloudError, CloudPath, NewSelfManagedTenantParams, StoredTenant, TenantStore, get_cloud_paths,
};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_manifest::HostingProvider;

use crate::cli_settings::CliConfig;
use crate::cloud::types::TenantDetailOutput;
use crate::shared::{CommandOutput, parse_tenant_id};

#[derive(Debug, Args)]
pub struct TenantAddArgs {
    #[arg(long, value_parser = parse_tenant_id, help = "Tenant id")]
    pub id: TenantId,

    #[arg(long, help = "Display name")]
    pub name: String,

    #[arg(long, help = "Public hostname the tenant is served on")]
    pub hostname: String,

    #[arg(long, help = "Hosting provider: kubernetes, gke or docker_local")]
    pub provider: HostingProvider,

    #[arg(long, help = "Region or cluster location")]
    pub region: Option<String>,

    #[arg(long, help = "Database URL reachable from this machine")]
    pub database_url: Option<String>,
}

pub fn add_tenant(args: TenantAddArgs, config: &CliConfig) -> Result<CommandOutput> {
    if args.provider.is_systemprompt() {
        bail!(
            "systemprompt.io tenants come from the control plane; run 'systemprompt cloud auth \
             login' or 'systemprompt cloud tenant create' instead"
        );
    }
    if args.hostname.trim().is_empty() {
        bail!("--hostname cannot be empty");
    }

    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    let mut store = match TenantStore::load_from_path(&tenants_path) {
        Ok(store) => store,
        Err(CloudError::TenantsNotSynced) => TenantStore::default(),
        Err(e) => return Err(e.into()),
    };

    let tenant = StoredTenant::new_self_managed(NewSelfManagedTenantParams {
        id: args.id,
        name: args.name,
        provider: args.provider,
        hostname: args.hostname.trim().to_owned(),
        region: args.region,
        database_url: args.database_url,
    });
    let output = TenantDetailOutput {
        id: tenant.id.clone(),
        name: tenant.name.clone(),
        provider: tenant.provider,
        app_id: None,
        hostname: tenant.hostname.clone(),
        region: tenant.region.clone(),
        has_database: tenant.has_database_url(),
    };

    store.insert_self_managed(tenant)?;
    store.save_to_path(&tenants_path)?;

    if !config.is_json_output() {
        CliService::success(&format!(
            "Added {} tenant '{}' ({})",
            output.provider, output.name, output.id
        ));
    }

    Ok(CommandOutput::card_value(format!("Tenant: {}", output.name), &output).with_skip_render())
}
