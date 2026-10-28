//! `cloud fleet versions`: each tenant's core version and readiness.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use serde::Serialize;
use systemprompt_cloud::deploy::provider::{probe_tenant, tenant_base_url};
use systemprompt_identifiers::TenantId;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};

use super::hosted_tenants;
use crate::shared::CommandOutput;

#[derive(Debug, Serialize)]
struct TenantVersionRow {
    tenant_id: TenantId,
    name: String,
    provider: String,
    url: String,
    version: Option<String>,
    ready: bool,
    status: String,
}

pub(super) async fn execute() -> Result<CommandOutput> {
    let client = reqwest::Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_DEFAULT_TIMEOUT)
        .build()?;
    let mut rows = Vec::new();
    for tenant in hosted_tenants()? {
        let Some(hostname) = tenant.hostname.as_deref() else {
            continue;
        };
        let health = probe_tenant(&client, &tenant_base_url(hostname)).await;
        rows.push(TenantVersionRow {
            tenant_id: tenant.id.clone(),
            name: tenant.name.clone(),
            provider: tenant.provider.to_string(),
            url: health.url,
            version: health.version,
            ready: health.ready,
            status: health.status,
        });
    }
    Ok(CommandOutput::table_of(
        vec!["name", "provider", "version", "ready", "status", "url"],
        &rows,
    ))
}
