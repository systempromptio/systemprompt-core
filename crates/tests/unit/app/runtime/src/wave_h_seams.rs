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
    DynSessionContextIssuer, ServiceProcessError, ServiceProcessKind, ServiceProcessSet,
    ServiceProcessStatus,
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
    assert!(
        state.find::<ActiveProfilePath>().is_some(),
        "kernel code reads it"
    );
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

    async fn statuses(&self) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError> {
        Ok(Vec::new())
    }

    async fn start(&self, _name: Option<&ServiceName>) -> Result<(), ServiceProcessError> {
        Ok(())
    }

    async fn stop(&self, _name: Option<&ServiceName>) -> Result<(), ServiceProcessError> {
        Ok(())
    }

    async fn restart(&self, _name: &ServiceName) -> Result<(), ServiceProcessError> {
        Ok(())
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
