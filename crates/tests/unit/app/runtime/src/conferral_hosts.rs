//! Every install conferral answers a member that declared it on every host
//! kind: issued on the API kernel (the one kernel every API, gateway,
//! worker, scheduler and CLI command process builds) and
//! `ConferralError::Withheld` on a child host, never missing. The row list
//! is `INSTALL_CONFERRALS` and the read is an exhaustive match, so a new row
//! fails here until it is seeded.

use std::sync::Arc;

use systemprompt_extension::reserved::{INSTALL_CONFERRALS, InstallConferral};
use systemprompt_extension::{ConferralError, ProvideError, Resolved};
use systemprompt_runtime::{AdmitFn, HostKernel, Kernel};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, host_child_env, host_probe_admitted,
    probe_admitted, seed_fixture_system_admin, test_database_url, test_db_pool,
};
use systemprompt_traits::BackgroundTasks;

#[derive(Clone, Debug)]
struct Answer(&'static str, Option<ConferralError>);

fn read(r: &Resolved<'_>, row: InstallConferral) -> Option<ConferralError> {
    match row {
        InstallConferral::MemberCipher => r.member_cipher().err(),
        InstallConferral::ShareTokens => r.share_tokens().err(),
        InstallConferral::GitSourceSync => r.git_source_sync().err(),
        InstallConferral::ScopedTokens => r.token_minter().err(),
        InstallConferral::BrokerCalls => r.broker_calls().err(),
        InstallConferral::AdmittedModels => r.admitted_models().err(),
    }
}

fn probes() -> Vec<Probe> {
    vec![Probe::new("conferral-hosts", |c| {
        for row in INSTALL_CONFERRALS {
            c.conferral(*row);
        }
        c.token_audience("conferral-hosts:probe", &["probe.read"]);
        c.provide_after_governance(|r: &Resolved<'_>| -> Result<Vec<Answer>, ProvideError> {
            Ok(INSTALL_CONFERRALS
                .iter()
                .map(|row| Answer(row.name(), read(r, *row)))
                .collect())
        });
    })]
}

#[tokio::test]
async fn the_api_kernel_issues_every_declared_conferral() {
    ensure_test_bootstrap();
    let ctx = systemprompt_test_fixtures::test_app_context(&test_database_url()).await;
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel
        .resolve(&probe_admitted(ComposedParts {
            probes: probes(),
            ..ComposedParts::default()
        }))
        .expect("resolves");
    let answers = kernel.state().get::<Vec<Answer>>().expect("provided");
    assert_eq!(answers.len(), INSTALL_CONFERRALS.len());
    for Answer(row, refused) in answers.iter() {
        assert!(
            refused.is_none(),
            "{row} is issued on the API kernel: {refused:?}"
        );
    }
}

#[tokio::test]
async fn a_child_host_withholds_every_declared_conferral() {
    let bootstrap = ensure_test_bootstrap();
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let env = host_child_env(bootstrap);
    let admit: AdmitFn = Box::new(|_profile| Ok(host_probe_admitted(probes())));
    let parts = HostKernel::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        admit,
    )
    .await
    .unwrap_or_else(|e| panic!("the child boots: {e:#}"));
    let answers: Arc<Vec<Answer>> = Arc::new(
        parts
            .kernel_state()
            .get::<Vec<Answer>>()
            .expect("provided")
            .clone(),
    );
    for Answer(row, refused) in answers.iter() {
        assert!(
            matches!(refused, Some(ConferralError::Withheld(_))),
            "{row} is withheld on a child host, never missing: {refused:?}"
        );
    }
}
