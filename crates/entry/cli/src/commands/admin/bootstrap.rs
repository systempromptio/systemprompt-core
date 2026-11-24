//! `admin bootstrap` command: ensures the system-admin user row exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::Config;
use systemprompt_traits::{AdminAccount, AdminBootstrap, AdminGrant, DynAdminBootstrap};

use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub email: Option<String>,

    #[arg(long, default_value = "Platform Admin")]
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct BootstrapOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub created: bool,
    pub roles: Vec<String>,
    pub message: String,
}

pub async fn execute(args: BootstrapArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let config = crate::commands::admin::state::config(host)?;
    let name = resolve_admin_name(&config, args.name.as_deref())?;

    let email = resolve_admin_email(&config, args.email.as_deref())?;

    let admins = connect_admin_bootstrap(host, &config).await?;

    let (user, created) = if let Some(existing) = admins.find_by_name(&name).await? {
        (existing, false)
    } else {
        let Some(email) = email else {
            return Err(anyhow!(
                "No admin email configured for '{name}'. Set `system_admin.email` in the profile \
                 or pass --email."
            ));
        };
        let created = admins
            .create(&name, email.as_str(), Some(&args.full_name))
            .await?;
        (created, true)
    };

    if !user.active {
        return Err(anyhow!(
            "Bootstrap user '{}' exists but has status '{}'; expected 'active'. Re-activate it \
             before running the platform.",
            user.name,
            user.status,
        ));
    }

    let user = ensure_admin_role(admins.as_ref(), user).await?;

    Ok(build_output(user, created))
}

fn resolve_admin_email(
    config: &Config,
    requested: Option<&str>,
) -> Result<Option<systemprompt_identifiers::Email>> {
    requested
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map_or_else(
            || Ok(config.system_admin_email.clone()),
            |e| {
                systemprompt_identifiers::Email::try_new(e)
                    .map(Some)
                    .map_err(|err| anyhow!("invalid --email '{e}': {err}"))
            },
        )
}

fn resolve_admin_name(config: &Config, requested: Option<&str>) -> Result<String> {
    let configured = config.system_admin_username.clone();
    if configured.trim().is_empty() {
        return Err(anyhow!(
            "Profile is missing `system_admin.username`; cannot run bootstrap"
        ));
    }

    match requested {
        Some(n) if !n.trim().is_empty() => {
            if n != configured {
                return Err(anyhow!(
                    "--name '{}' does not match profile system_admin.username '{}'; refusing to \
                     bootstrap the wrong user",
                    n,
                    configured,
                ));
            }
            Ok(n.to_owned())
        },
        _ => Ok(configured),
    }
}

async fn connect_admin_bootstrap(
    host: &dyn KernelHost,
    config: &Config,
) -> Result<DynAdminBootstrap> {
    let database: DbPool = Arc::new(
        Database::connect(
            &config.database_url,
            config.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await
        .context("Failed to connect to database")?,
    );
    Ok(systemprompt_runtime::admin_bootstrap(
        host.admitted()?,
        database,
    )?)
}

async fn ensure_admin_role(
    admins: &dyn AdminBootstrap,
    user: AdminAccount,
) -> Result<AdminAccount> {
    let user = if user.is_admin() {
        user
    } else {
        admins.grant_admin(&user.id, AdminGrant::Added).await?
    };

    if !user.is_admin() {
        return Err(anyhow!(
            "Failed to assign 'admin' role to bootstrap user '{}'",
            user.name
        ));
    }

    Ok(user)
}

fn build_output(user: AdminAccount, created: bool) -> CommandOutput {
    let message = if created {
        format!(
            "Bootstrap user '{}' created and granted admin role",
            user.name
        )
    } else {
        format!(
            "Bootstrap user '{}' already exists; admin role verified",
            user.name
        )
    };

    let output = BootstrapOutput {
        id: user.id,
        name: user.name,
        email: user.email,
        created,
        roles: user.roles,
        message,
    };

    let title = if created {
        "Admin Bootstrapped"
    } else {
        "Admin Verified"
    };
    CommandOutput::card_value(title, &output)
}
