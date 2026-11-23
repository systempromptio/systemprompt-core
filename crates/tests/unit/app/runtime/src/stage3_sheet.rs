//! Stage 3 dry-run sheet: for every route group, job and command that moves
//! or changes owner this stage, the exact value the moved code reads resolves
//! from the context it will run in, and a value the rules withhold is refused
//! there.

use std::sync::Arc;

use systemprompt_ai::PersonalProviderCredentials;
use systemprompt_ai::repository::AiRepositories;
use systemprompt_database::DbPool;
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_extension::{DynBrainSink, Resolved, StandingTag};
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_gateway::GatewayEngines;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Config;
use systemprompt_mcp::ArtifactIngest;
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_models::auth::{DynRequestAuthenticator, DynTokenRevocations, TokenAuthority};
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, CacheEpochView, CacheEpochs,
    DynAiSessionProvider, DynContextDirectory, DynGatewaySessionMint, DynSessionContextIssuer,
    DynSessionStore, DynSessionTelemetry, DynSessionUsageCounters, FileStorage, SecretStore,
};

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn command_kernel(ctx: &AppContext) -> Kernel {
    Kernel::new(
        AppContextTrait::database_handle(ctx),
        BackgroundTasks::new(),
        ctx.kernel().state().clone(),
    )
}

fn route_reads<T: Clone + Send + Sync + 'static>(ctx: &AppContext, owner: &str) -> bool {
    ctx.kernel()
        .route_scoped(&ExtensionId::new(owner), StandingTag::Core, &[])
        .get::<T>()
        .is_ok()
}

fn install_reads<T: Clone + Send + Sync + 'static>(ctx: &AppContext) -> bool {
    ctx.kernel()
        .route_scoped(&ExtensionId::new("an-install"), StandingTag::Install, &[])
        .get::<T>()
        .is_ok()
}

fn member_command_reads<T: Clone + Send + Sync + 'static>(ctx: &AppContext) -> bool {
    let kernel = command_kernel(ctx);
    Resolved::__member_command(&kernel, kernel.state())
        .optional::<T>()
        .is_some()
}

fn job_reads<T: Clone + Send + Sync + 'static>(ctx: &AppContext) -> bool {
    command_kernel(ctx).runtime_state().find::<T>().is_some()
}

#[tokio::test]
async fn gateway_route_set_reads_its_own_sealed_engines() {
    let ctx = context().await;
    assert!(route_reads::<Arc<GatewayEngines>>(&ctx, "gateway"));
}

#[tokio::test]
async fn gateway_http_reads_the_engines_under_its_conferral_row() {
    let ctx = context().await;
    assert!(route_reads::<Arc<GatewayEngines>>(&ctx, "gateway-http"));
}

#[tokio::test]
async fn gateway_engines_are_refused_to_every_other_route_set_command_and_job() {
    let ctx = context().await;
    for owner in ["identity-http", "files-http", "agent-http", "mcp-http"] {
        assert!(!route_reads::<Arc<GatewayEngines>>(&ctx, owner), "{owner}");
    }
    assert!(!install_reads::<Arc<GatewayEngines>>(&ctx));
    assert!(!member_command_reads::<Arc<GatewayEngines>>(&ctx));
    assert!(!job_reads::<Arc<GatewayEngines>>(&ctx));
}

#[tokio::test]
async fn gateway_route_set_reads_the_open_values_its_repositories_are_built_from() {
    let ctx = context().await;
    assert!(route_reads::<DbPool>(&ctx, "gateway"));
    assert!(route_reads::<DynSessionTelemetry>(&ctx, "gateway"));
    assert!(route_reads::<DynSessionUsageCounters>(&ctx, "gateway"));
    assert!(route_reads::<Arc<ArtifactIngest>>(&ctx, "gateway"));
    assert!(route_reads::<DynBrainSink>(&ctx, "gateway"));
    assert!(route_reads::<DynContextDirectory>(&ctx, "gateway"));
}

#[tokio::test]
async fn gateway_route_set_is_refused_the_users_and_ai_sealed_provides() {
    let ctx = context().await;
    assert!(!route_reads::<DynSessionStore>(&ctx, "gateway"));
    assert!(!route_reads::<DynAiSessionProvider>(&ctx, "gateway"));
    assert!(!route_reads::<Arc<AiRepositories>>(&ctx, "gateway"));
}

fn conferred_on_gateway_http_alone<T: Clone + Send + Sync + 'static>(
    ctx: &AppContext,
    provider: &str,
) {
    assert!(route_reads::<T>(ctx, "gateway-http"));
    assert!(route_reads::<T>(ctx, provider));
    for owner in [
        "gateway",
        "identity-http",
        "files-http",
        "agent-http",
        "mcp-http",
    ] {
        if owner != provider {
            assert!(!route_reads::<T>(ctx, owner), "{owner}");
        }
    }
    assert!(!install_reads::<T>(ctx));
    assert!(!member_command_reads::<T>(ctx));
    assert!(!job_reads::<T>(ctx));
}

#[tokio::test]
async fn gateway_http_alone_reads_the_conferred_session_mint() {
    let ctx = context().await;
    conferred_on_gateway_http_alone::<DynGatewaySessionMint>(&ctx, "users");
    assert!(!route_reads::<DynSessionStore>(&ctx, "gateway-http"));
}

#[tokio::test]
async fn gateway_http_alone_reads_the_conferred_personal_credential_view() {
    let ctx = context().await;
    conferred_on_gateway_http_alone::<Arc<PersonalProviderCredentials>>(&ctx, "ai");
}

#[tokio::test]
async fn gateway_http_builds_its_rejection_records_from_open_values_only() {
    let ctx = context().await;
    assert!(route_reads::<DbPool>(&ctx, "gateway-http"));
    assert!(route_reads::<DynSessionUsageCounters>(&ctx, "gateway-http"));
    assert!(!route_reads::<Arc<AiRepositories>>(&ctx, "gateway-http"));
}

#[tokio::test]
async fn every_context_reads_the_open_cache_epoch_view_and_no_member_context_advances_epochs() {
    let ctx = context().await;
    for owner in ["gateway-http", "identity-http", "files-http", "gateway"] {
        assert!(route_reads::<CacheEpochView>(&ctx, owner), "{owner}");
    }
    assert!(install_reads::<CacheEpochView>(&ctx));
    assert!(member_command_reads::<CacheEpochView>(&ctx));
    assert!(job_reads::<CacheEpochView>(&ctx));
    assert!(!install_reads::<CacheEpochs>(&ctx));
    assert!(!member_command_reads::<CacheEpochs>(&ctx));
    assert!(!job_reads::<CacheEpochs>(&ctx));
}

#[tokio::test]
async fn the_seeded_epoch_view_reads_the_epochs_the_kernel_advances() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    let epochs = state.get::<CacheEpochs>().expect("sealed epochs seeded");
    let view = state.get::<CacheEpochView>().expect("open view seeded");
    let domain = systemprompt_traits::CacheDomain::GatewayPolicy;
    let before = view.current(domain);
    epochs.advance(domain);
    assert_eq!(view.current(domain), before + 1);
}

#[tokio::test]
async fn the_token_authority_is_one_sealed_kernel_seed_conferred_on_identity_http() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    let seeded = state
        .get::<Arc<dyn TokenAuthority>>()
        .expect("the API seeds the token authority");
    assert!(Arc::ptr_eq(seeded, &ctx.token_authority()));
    assert!(state.is_sealed::<Arc<dyn TokenAuthority>>());
    let conferred = state
        .conferral_of::<Arc<dyn TokenAuthority>>()
        .expect("recorded under its row");
    assert_eq!(conferred.row, ConferralRow::IdentityTokenAuthority.name());
    assert_eq!(conferred.holder.as_str(), "identity-http");
    assert!(route_reads::<Arc<dyn TokenAuthority>>(
        &ctx,
        "identity-http"
    ));
    assert!(!install_reads::<Arc<dyn TokenAuthority>>(&ctx));
    assert!(!member_command_reads::<Arc<dyn TokenAuthority>>(&ctx));
    assert!(!job_reads::<Arc<dyn TokenAuthority>>(&ctx));
}

#[tokio::test]
async fn every_route_set_command_and_job_reads_the_open_request_authenticator() {
    let ctx = context().await;
    for owner in ["gateway-http", "identity-http", "files-http", "agent-http"] {
        assert!(
            route_reads::<DynRequestAuthenticator>(&ctx, owner),
            "{owner}"
        );
    }
    assert!(install_reads::<DynRequestAuthenticator>(&ctx));
    assert!(member_command_reads::<DynRequestAuthenticator>(&ctx));
    assert!(job_reads::<DynRequestAuthenticator>(&ctx));
    assert!(route_reads::<DynTokenRevocations>(&ctx, "identity-http"));
    assert!(!ctx.kernel().state().is_sealed::<DynRequestAuthenticator>());
}

#[tokio::test]
async fn users_route_set_alone_reads_the_sealed_session_store() {
    let ctx = context().await;
    assert!(route_reads::<DynSessionStore>(&ctx, "users"));
    for owner in ["gateway", "files-http", "agent-http", "marketplace-http"] {
        assert!(!route_reads::<DynSessionStore>(&ctx, owner), "{owner}");
    }
    assert!(!member_command_reads::<DynSessionStore>(&ctx));
    assert!(!job_reads::<DynSessionStore>(&ctx));
}

#[tokio::test]
async fn documents_route_set_reads_the_files_config_handles_and_storage() {
    let ctx = context().await;
    for owner in ["files", "files-http"] {
        assert!(route_reads::<Arc<FilesConfig>>(&ctx, owner), "{owner}");
        assert!(route_reads::<DocumentsHandlesSlot>(&ctx, owner), "{owner}");
        assert!(route_reads::<Arc<dyn FileStorage>>(&ctx, owner), "{owner}");
    }
}

#[tokio::test]
async fn documents_and_issues_jobs_and_commands_read_the_documents_handles() {
    let ctx = context().await;
    assert!(job_reads::<DocumentsHandlesSlot>(&ctx));
    assert!(job_reads::<Arc<FilesConfig>>(&ctx));
    assert!(member_command_reads::<DocumentsHandlesSlot>(&ctx));
}

#[tokio::test]
async fn agent_member_reads_the_dispatch_cell_in_provide_and_its_route_set_the_context_seams() {
    let ctx = context().await;
    let agent = ExtensionId::new("agent-http");
    assert!(
        ctx.kernel()
            .member_resolved(&agent, StandingTag::Core)
            .get::<AgentDispatchCell>()
            .is_ok(),
        "the agent member's provide fns read the sealed dispatch cell"
    );
    assert!(
        !route_reads::<AgentDispatchCell>(&ctx, "agent-http"),
        "no route state reads a host-sealed seed"
    );
    assert!(route_reads::<DynContextDirectory>(&ctx, "agent-http"));
    assert!(route_reads::<DynSessionContextIssuer>(&ctx, "agent-http"));
}

#[tokio::test]
async fn agent_dispatch_is_refused_to_member_commands_jobs_and_installs() {
    let ctx = context().await;
    assert!(!member_command_reads::<AgentDispatchCell>(&ctx));
    assert!(!job_reads::<AgentDispatchCell>(&ctx));
    assert!(!install_reads::<AgentDispatchCell>(&ctx));
}

#[tokio::test]
async fn runtime_context_seams_resolve_for_jobs_and_member_commands() {
    let ctx = context().await;
    assert!(job_reads::<DynContextDirectory>(&ctx));
    assert!(job_reads::<DynSessionContextIssuer>(&ctx));
    assert!(member_command_reads::<DynContextDirectory>(&ctx));
    assert!(job_reads::<DynSessionTelemetry>(&ctx));
}

#[tokio::test]
async fn core_and_install_route_sets_are_refused_every_host_sealed_seed() {
    let ctx = context().await;
    for owner in [
        "content-http",
        "marketplace-http",
        "agent_runs_api",
        "slack",
        "teams",
    ] {
        assert!(!route_reads::<Arc<Config>>(&ctx, owner), "{owner}");
        assert!(!route_reads::<Arc<dyn SecretStore>>(&ctx, owner), "{owner}");
    }
    assert!(!install_reads::<Arc<Config>>(&ctx));
    assert!(!install_reads::<Arc<dyn SecretStore>>(&ctx));
}
