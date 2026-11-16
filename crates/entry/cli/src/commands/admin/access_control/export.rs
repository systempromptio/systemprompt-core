//! `admin access-control export-yaml`: the database's declared access as
//! `services/access-control/rules.yaml`, the same rendering the
//! `access_control` config plane exports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_governance::authz::render_current_rules;

use crate::shared::CommandOutput;
use super::ExportYamlArgs;

pub(super) async fn run(_args: ExportYamlArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let yaml = render_yaml_snapshot(&pool).await?;
    Ok(CommandOutput::text_titled(
        "Access control (paste into services/access-control/rules.yaml)",
        yaml,
    ))
}

pub async fn render_yaml_snapshot(pool: &DbPool) -> Result<String> {
    Ok(render_current_rules(pool).await?)
}
