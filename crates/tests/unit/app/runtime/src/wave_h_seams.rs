//! The Wave H seams keep their seal: the API's agent dispatch cell and the
//! supervised process sets are sealed (kernel code and core slot builds read
//! them, no member command resolver or runtime context does), while the
//! conversation-context seams the agent member provides are open.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::service_process::{ServiceProcessSets, ServiceProcessSlot};
use systemprompt_extension::{Resolved, StandingTag};
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::root::ActiveProfilePath;
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynContextDirectory, DynServiceProcessSet,
    DynSessionContextIssuer, RestartMode, ServiceProcessError, ServiceProcessKind,
    ServiceProcessSet, ServiceProcessStatus, ServiceRun, StatusDepth, StopMode,
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

#[tokio::test]
async fn the_agent_dispatch_cell_is_sealed_from_member_code() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<AgentDispatchCell>());
    assert!(
        state.find::<AgentDispatchCell>().is_some(),
        "kernel code reads it"
    );
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<AgentDispatchCell>().is_none());
    assert!(
        kernel.runtime_state().find::<AgentDispatchCell>().is_none(),
        "no job, runner or plane context reads it"
    );
}

#[tokio::test]
async fn the_spawner_and_the_profile_path_are_sealed_from_member_code() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<Arc<systemprompt_loader::subprocess::Spawner>>());
    assert!(state.is_sealed::<ActiveProfilePath>());
    assert!(state.find::<ActiveProfilePath>().is_some(), "kernel code reads it");
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(
        member
            .optional::<Arc<systemprompt_loader::subprocess::Spawner>>()
            .is_none()
    );
    assert!(member.optional::<ActiveProfilePath>().is_none());
    let runtime = kernel.runtime_state();
    assert!(
        runtime
            .find::<Arc<systemprompt_loader::subprocess::Spawner>>()
            .is_none()
    );
    assert!(runtime.find::<ActiveProfilePath>().is_none());
}

#[tokio::test]
async fn the_context_seams_are_open_and_served_to_member_commands() {
    let ctx = context().await;
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<DynContextDirectory>().is_some());
    assert!(member.optional::<DynSessionContextIssuer>().is_some());
}

struct StubSet;

#[async_trait]
impl ServiceProcessSet for StubSet {
    fn kind(&self) -> ServiceProcessKind {
        ServiceProcessKind::Agent
    }

    async fn statuses(
        &self,
        _depth: StatusDepth,
    ) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError> {
        Ok(Vec::new())
    }

    async fn start(
        &self,
        _name: Option<&ServiceName>,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        Ok(Vec::new())
    }

    async fn stop(
        &self,
        _name: Option<&ServiceName>,
        _mode: StopMode,
    ) -> Result<(), ServiceProcessError> {
        Ok(())
    }

    async fn restart(
        &self,
        _name: Option<&ServiceName>,
        _mode: RestartMode,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn the_service_process_sets_enter_the_state_sealed() {
    let ctx = context().await;
    let kernel = command_kernel(&ctx);
    let mut builder = systemprompt_extension::service_process::ServiceProcessBuilder::default();
    builder.process_set(|_: &Resolved<'_>| -> Result<DynServiceProcessSet, _> {
        Ok(Arc::new(StubSet))
    });
    let mut state = systemprompt_provider_contracts::Dependencies::default();
    let built = systemprompt_test_fixtures::slots::built_slot_into::<ServiceProcessSlot>(
        vec![("agent", StandingTag::Core, builder)],
        &kernel.resolved(),
        &mut state,
    )
    .expect("a core entry builds the slot");
    assert_eq!(built.sets().len(), 1);
    assert!(state.is_sealed::<Arc<ServiceProcessSets>>());
    assert!(
        state
            .withholding_sealed()
            .find::<Arc<ServiceProcessSets>>()
            .is_none(),
        "every runtime context is refused the process sets"
    );
}

#[tokio::test]
async fn the_child_secret_env_and_the_process_sets_are_sealed_from_member_code() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<systemprompt_config::DynChildSecretEnv>());
    assert!(state.is_sealed::<Arc<ServiceProcessSets>>());
    let sets = state
        .find::<Arc<ServiceProcessSets>>()
        .expect("kernel code reads the process sets");
    assert!(sets.of_kind(ServiceProcessKind::Agent).is_some());
    assert!(sets.of_kind(ServiceProcessKind::McpServer).is_some());
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<systemprompt_config::DynChildSecretEnv>().is_none());
    assert!(member.optional::<Arc<ServiceProcessSets>>().is_none());
    let runtime = kernel.runtime_state();
    assert!(runtime.find::<systemprompt_config::DynChildSecretEnv>().is_none());
    assert!(runtime.find::<Arc<ServiceProcessSets>>().is_none());
}

#[tokio::test]
async fn a_disabled_member_removes_its_kind_and_leaves_the_others() {
    ensure_test_bootstrap();
    let ctx = systemprompt_test_fixtures::fixture_app_context_with_extensions(
        systemprompt_test_fixtures::fixture_config(&test_database_url()),
        systemprompt_test_fixtures::fixture_admitted_without(&[
            systemprompt_identifiers::ExtensionId::new("agent"),
        ]),
    )
    .await
    .expect("the composition without the agent member builds");
    let sets = Arc::clone(
        ctx.kernel()
            .state()
            .find::<Arc<ServiceProcessSets>>()
            .expect("the MCP member still opens the slot"),
    );
    assert!(sets.of_kind(ServiceProcessKind::Agent).is_none());
    let mcp = sets
        .of_kind(ServiceProcessKind::McpServer)
        .expect("the MCP set is unaffected");
    mcp.statuses(StatusDepth::Listed)
        .await
        .expect("the MCP set still answers");
}
