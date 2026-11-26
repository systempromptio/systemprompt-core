//! An install member composed after the core members reads the
//! `admitted-models` conferral from its early provide: on the API kernel the
//! read is a handle, so the install's state is built rather than the
//! composition refused; on a child host it is `Withheld`, which withholds
//! the install's provide in turn.

use systemprompt_extension::artifact_ui::HostRole;
use systemprompt_extension::reserved::InstallConferral;
use systemprompt_extension::{ConferralError, Extension, ProvideError, Resolved};
use systemprompt_runtime::{AdmitFn, HostKernel};
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, fixture_admitted_of, fixture_app_context_with_extensions,
    fixture_config, fixture_core_members, host_child_env, host_probe_admitted,
    seed_fixture_system_admin, test_db_pool,
};

#[derive(Clone, Debug)]
struct InstallState(Option<ConferralError>);

fn install() -> Probe {
    Probe::new("models-install", |c| {
        c.conferral(InstallConferral::AdmittedModels);
        c.provide(|r: &Resolved<'_>| -> Result<InstallState, ProvideError> {
            match r.admitted_models() {
                Ok(_) => Ok(InstallState(None)),
                Err(ConferralError::Withheld(row)) => Err(ConferralError::Withheld(row).into()),
                Err(other) => Ok(InstallState(Some(other))),
            }
        });
    })
}

#[tokio::test]
async fn on_the_api_kernel_an_early_install_provide_is_issued_the_handle() {
    let boot = ensure_test_bootstrap();
    let members: Vec<Box<dyn Extension>> = fixture_core_members()
        .into_iter()
        .chain(std::iter::once(Box::new(install()) as Box<dyn Extension>))
        .collect();
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&boot.database_url),
        fixture_admitted_of(members),
    )
    .await
    .expect("a composition with gateway-http and an install reading admitted-models builds");
    let state = ctx
        .kernel()
        .state()
        .get::<InstallState>()
        .expect("the install state is provided");
    assert!(state.0.is_none(), "issued, not refused: {:?}", state.0);
}

#[tokio::test]
async fn on_a_child_host_the_install_provide_is_withheld_not_refused() {
    let bootstrap = ensure_test_bootstrap();
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let env = host_child_env(bootstrap);
    let admit: AdmitFn = Box::new(|_profile| Ok(host_probe_admitted(vec![install()])));
    let parts = HostKernel::boot(
        HostRole::McpServer,
        move |name| env.get(name).cloned(),
        admit,
    )
    .await
    .expect("the child host boots with the install withheld");
    assert!(
        parts.kernel_state().find::<InstallState>().is_none(),
        "the install's provide is withheld on a child host"
    );
}
