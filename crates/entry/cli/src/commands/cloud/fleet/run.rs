//! `cloud fleet run`: forward one read-only command to every tenant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use systemprompt_client::{OutputSink, RemoteCliExecutor, RemoteCliRequest};
use systemprompt_cloud::deploy::provider::tenant_base_url;
use systemprompt_cloud::{CloudPath, SessionKey, SessionStore, StoredTenant, get_cloud_paths};
use systemprompt_identifiers::{ContextId, SessionToken, TenantId};
use systemprompt_logging::CliService;

use super::{FleetRunArgs, ensure_read_only, hosted_tenants};
use crate::cli_settings::CliConfig;
use crate::runner::routing::operator_tenant_token;

#[derive(Debug, Default)]
struct CaptureSink {
    stdout: String,
    stderr: String,
    errors: Vec<String>,
}

impl OutputSink for CaptureSink {
    fn stdout_chunk(&mut self, data: &str) -> io::Result<()> {
        self.stdout.push_str(data);
        Ok(())
    }

    fn stderr_chunk(&mut self, data: &str) -> io::Result<()> {
        self.stderr.push_str(data);
        Ok(())
    }

    fn error_message(&mut self, message: &str) {
        self.errors.push(message.to_owned());
    }
}

#[derive(Debug, Serialize)]
struct TenantRun {
    tenant_id: TenantId,
    tenant: String,
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct FleetRunOutput {
    command: Vec<String>,
    tenants: Vec<TenantRun>,
}

pub(super) async fn execute(args: FleetRunArgs, config: &CliConfig) -> Result<()> {
    ensure_read_only(&args.command)?;

    let mut forwarded = args.command.clone();
    if config.is_json_output() && !forwarded.iter().any(|arg| arg == "--json") {
        forwarded.push("--json".to_owned());
    }

    let tenants = hosted_tenants()?;
    let mut runs = Vec::with_capacity(tenants.len());
    for tenant in &tenants {
        runs.push(run_on_tenant(tenant, &forwarded, args.timeout, config).await);
    }

    let failed = runs.iter().filter(|run| run.exit_code != Some(0)).count();
    if config.is_json_output() {
        CliService::json(&FleetRunOutput {
            command: args.command,
            tenants: runs,
        });
    }
    if failed > 0 {
        bail!("{failed} of {} tenant(s) failed", tenants.len());
    }
    Ok(())
}

async fn tenant_credentials(
    tenant: &StoredTenant,
    hostname: &str,
) -> Result<(SessionToken, ContextId)> {
    if tenant.provider.is_self_managed() {
        return Ok((
            operator_tenant_token(hostname).await?,
            ContextId::generate(),
        ));
    }
    let sessions_dir = get_cloud_paths().resolve(CloudPath::SessionsDir);
    let store = SessionStore::load_or_create(&sessions_dir)?;
    let session = store
        .get_session(&SessionKey::Tenant(tenant.id.clone()))
        .filter(|session| !session.is_expired())
        .context("No active session. Run 'systemprompt admin session login' for this tenant.")?;
    Ok((session.session_token.clone(), session.context_id.clone()))
}

async fn run_on_tenant(
    tenant: &StoredTenant,
    args: &[String],
    timeout_secs: u64,
    config: &CliConfig,
) -> TenantRun {
    let mut run = TenantRun {
        tenant_id: tenant.id.clone(),
        tenant: tenant.name.clone(),
        exit_code: None,
        output: None,
        error: None,
    };
    let Some(hostname) = tenant.hostname.as_deref() else {
        run.error = Some("tenant has no hostname".to_owned());
        return run;
    };
    let mut sink = CaptureSink::default();
    let outcome = async {
        let (token, context) = tenant_credentials(tenant, hostname).await?;
        let executor = RemoteCliExecutor::new(&tenant_base_url(hostname), timeout_secs)?;
        let request = RemoteCliRequest {
            token: &token,
            context: Some(&context),
            args,
        };
        anyhow::Ok(executor.execute(request, &mut sink).await?)
    }
    .await;

    match outcome {
        Ok(code) => run.exit_code = Some(code),
        Err(error) => run.error = Some(format!("{error:#}")),
    }
    if !sink.errors.is_empty() && run.error.is_none() {
        run.error = Some(sink.errors.join("; "));
    }

    if config.is_json_output() {
        run.output = Some(
            serde_json::from_str(sink.stdout.trim())
                .unwrap_or_else(|_| serde_json::Value::String(sink.stdout.clone())),
        );
    } else {
        print_prefixed(&tenant.name, &sink, &run);
    }
    run
}

fn print_prefixed(name: &str, sink: &CaptureSink, run: &TenantRun) {
    for line in sink.stdout.lines() {
        CliService::output(&format!("[{name}] {line}"));
    }
    for line in sink.stderr.lines() {
        CliService::warning(&format!("[{name}] {line}"));
    }
    if let Some(error) = &run.error {
        CliService::error(&format!("[{name}] {error}"));
    } else if let Some(code) = run.exit_code.filter(|code| *code != 0) {
        CliService::error(&format!("[{name}] exited with code {code}"));
    }
}
