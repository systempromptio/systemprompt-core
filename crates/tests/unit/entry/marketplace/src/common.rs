//! Shared helpers for the marketplace tests: the marketplace route set
//! materialized over a fixture context's resolved kernel state, as the API
//! mount materializes it, and command contexts the marketplace commands run
//! on — the detached command kernel over the bootstrap profile, or an
//! application context — served through a member host holding the
//! services-tree grant.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{
    CliConfig, CommandDescriptor, CommandError, CommandHost, CommandOutput, KernelGrant,
    OutputFormat,
};
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_marketplace_entry::http::public_routes;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_admitted};

pub fn marketplace_router(ctx: &AppContext) -> Router {
    public_routes()
        .materialize(&ctx.kernel().resolved())
        .expect("the marketplace state resolves from the fixture kernel")
        .into_iter()
        .fold(Router::new(), Router::merge)
}

pub async fn body_to_string(resp: Response<Body>) -> Result<(http::StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

pub fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}

pub fn json_cli() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn booted(boot: &BootContext) -> BootContext {
    boot.install_services_root(boot.require_services_root());
    boot.clone()
}

pub fn services_tree_ctx() -> CommandContext {
    let boot = booted(&ensure_test_bootstrap().boot);
    let services = ServicesState::load(&boot).expect("the bootstrap services config loads");
    CommandContext::new(json_cli(), EnvOverrides::default())
        .with_boot(Some(boot))
        .with_admitted(Some(fixture_admitted()))
        .with_services(Some(services))
        .with_detached_kernel()
}

pub fn app_ctx(app: Arc<AppContext>) -> CommandContext {
    let boot = booted(app.boot());
    let admitted = Arc::clone(app.admitted());
    CommandContext::with_app_context(json_cli(), EnvOverrides::default(), app)
        .with_boot(Some(boot))
        .with_admitted(Some(admitted))
}

pub fn granted(ctx: &CommandContext) -> MemberHost<'_> {
    granted_with(ctx, CommandDescriptor::PROFILE_SECRETS_AND_PATHS)
}

pub fn granted_with(ctx: &CommandContext, descriptor: CommandDescriptor) -> MemberHost<'_> {
    MemberHost::new(ctx, descriptor.with_grant(KernelGrant::ServicesTree))
}

pub fn rendered(
    host: &dyn CommandHost,
    outcome: anyhow::Result<CommandOutput>,
) -> anyhow::Result<CommandOutput> {
    match &outcome {
        Ok(output) => host.render(output),
        Err(error) => {
            if let Some(output) = error
                .downcast_ref::<CommandError>()
                .and_then(CommandError::output)
            {
                host.render(output);
            }
        },
    }
    outcome
}
