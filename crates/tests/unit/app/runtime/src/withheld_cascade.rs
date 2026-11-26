//! A withheld provide withholds its dependents: on a child host a value
//! built from a withheld conferral is withheld, and so is every provide that
//! reads it, two levels deep, and the host boots. On the API host the same
//! chain is provided whole. A dependency nothing provides is still a missing
//! dependency and refuses the resolution on either host.

use std::sync::Arc;

use systemprompt_extension::reserved::InstallConferral;
use systemprompt_extension::{Admitted, ProvideError, Resolved};
use systemprompt_runtime::{
    AdmitFn, AppContext, HostBootError, HostKernel, HostParts, Kernel, KernelError,
};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, host_child_env, host_probe_admitted,
    probe_admitted, seed_fixture_system_admin, test_database_url, test_db_pool,
};
use systemprompt_traits::BackgroundTasks;

#[derive(Clone)]
struct Cipher;

#[derive(Clone)]
struct Level1;

#[derive(Clone)]
struct Level2;

#[derive(Clone)]
struct Absent;

#[derive(Clone)]
struct NeedsAbsent;

fn chain() -> Vec<Probe> {
    vec![
        Probe::new("cascade-root", |c| {
            c.conferral(InstallConferral::MemberCipher);
            c.provide(|r: &Resolved<'_>| -> Result<Cipher, ProvideError> {
                Ok(r.member_cipher().map(|_| Cipher)?)
            });
        }),
        Probe::new("cascade-one", |c| {
            c.provide(|r: &Resolved<'_>| -> Result<Level1, ProvideError> {
                r.get::<Cipher>()?;
                Ok(Level1)
            });
        }),
        Probe::new("cascade-two", |c| {
            c.provide(|r: &Resolved<'_>| -> Result<Level2, ProvideError> {
                r.get::<Level1>()?;
                Ok(Level2)
            });
        }),
    ]
}

fn needs_absent() -> Probe {
    Probe::new("cascade-absent", |c| {
        c.provide(|r: &Resolved<'_>| -> Result<NeedsAbsent, ProvideError> {
            r.get::<Absent>()?;
            Ok(NeedsAbsent)
        });
    })
}

async fn boot_child(probes: fn() -> Vec<Probe>) -> Result<HostParts, HostBootError> {
    let bootstrap = ensure_test_bootstrap();
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let env = host_child_env(bootstrap);
    let admit: AdmitFn = Box::new(move |_profile| Ok(host_probe_admitted(probes())));
    HostKernel::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        admit,
    )
    .await
}

#[tokio::test]
async fn a_withheld_value_withholds_its_dependents_two_levels_deep_on_a_child_host() {
    let parts = boot_child(chain)
        .await
        .unwrap_or_else(|e| panic!("the child host boots with the chain withheld: {e:#}"));
    let state = parts.kernel_state();
    assert!(state.find::<Cipher>().is_none(), "the root is withheld");
    assert!(
        state.find::<Level1>().is_none(),
        "its dependent is withheld"
    );
    assert!(state.find::<Level2>().is_none(), "and that one's dependent");
    assert!(state.is_withheld::<Level2>());
}

#[tokio::test]
async fn a_truly_missing_dependency_still_refuses_a_child_host() {
    let refused = boot_child(|| vec![needs_absent()]).await;
    assert!(
        refused.is_err(),
        "a dependency nothing provides is never read as withheld"
    );
}

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn admitted(probes: Vec<Probe>) -> Admitted {
    probe_admitted(ComposedParts {
        probes,
        ..ComposedParts::default()
    })
}

fn resolve(ctx: &AppContext, probes: Vec<Probe>) -> Result<Kernel, KernelError> {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel.resolve(&admitted(probes))?;
    Ok(kernel)
}

#[tokio::test]
async fn on_the_api_host_nothing_in_the_chain_is_withheld() {
    let ctx = context().await;
    let kernel = resolve(&ctx, chain()).expect("the chain resolves on the API host");
    let state = kernel.state();
    assert!(state.find::<Cipher>().is_some());
    assert!(state.find::<Level1>().is_some());
    assert!(state.find::<Level2>().is_some());
    assert!(!state.is_withheld::<Level2>());

    let error = resolve(&ctx, vec![needs_absent()])
        .err()
        .expect("a missing dependency refuses the API host");
    assert!(
        matches!(
            &error,
            KernelError::Provide { source: ProvideError::Missing(missing), .. }
                if !missing.is_withheld()
        ),
        "{error:?}"
    );
}
