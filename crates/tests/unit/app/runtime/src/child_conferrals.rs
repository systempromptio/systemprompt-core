//! A child host confers nothing. A member that declared a conferral boots
//! there with its value withheld and observes `ConferralError::Withheld`; a
//! member that declared nothing reads `Missing`, so no member reads another
//! member's capability; and the child holds no conferral issuer even when the
//! encryption master key was granted to it.

use std::sync::Arc;

use systemprompt_extension::reserved::InstallConferral;
use systemprompt_extension::{ConferralError, ConferralsWithheld, ProvideError, Resolved};
use systemprompt_models::auth::ScopedTokenMinter;
use systemprompt_runtime::{AdmitFn, HostKernel};
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, host_child_env, host_probe_admitted, seed_fixture_system_admin,
    test_db_pool,
};
use systemprompt_traits::{
    DynConferralIssuer, GitSourceSync, MemberCipher, SecretStore, ShareTokenSigner,
};

#[derive(Clone)]
struct Cipher;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observed(&'static str, Option<ConferralError>);

fn kind(read: Result<Arc<dyn MemberCipher>, ConferralError>) -> Option<ConferralError> {
    read.err()
}

fn admit() -> AdmitFn {
    Box::new(|_profile| {
        Ok(host_probe_admitted(vec![
            Probe::new("child-declared", |c| {
                c.conferral(InstallConferral::MemberCipher);
                c.provide(|r: &Resolved<'_>| -> Result<Cipher, ProvideError> {
                    Ok(r.member_cipher().map(|_| Cipher)?)
                });
            }),
            Probe::new("child-observer", |c| {
                c.conferral(InstallConferral::MemberCipher);
                c.provide(|r: &Resolved<'_>| -> Result<Observed, ProvideError> {
                    Ok(Observed("declared", kind(r.member_cipher())))
                });
            }),
            Probe::new("child-undeclared", |c| {
                c.provide(|r: &Resolved<'_>| -> Result<Vec<Observed>, ProvideError> {
                    Ok(vec![Observed("undeclared", kind(r.member_cipher()))])
                });
            }),
        ]))
    })
}

#[tokio::test]
async fn a_child_host_withholds_every_conferral_and_still_boots() {
    let bootstrap = ensure_test_bootstrap();
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let mut env = host_child_env(bootstrap);
    env.insert(
        "ENCRYPTION_MASTER_KEY".to_owned(),
        "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
    );
    env.insert(
        systemprompt_manifest::paths::constants::env_vars::CUSTOM_SECRETS.to_owned(),
        "ENCRYPTION_MASTER_KEY".to_owned(),
    );
    let parts = HostKernel::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        admit(),
    )
    .await
    .unwrap_or_else(|e| panic!("a child host with a declared conferral boots: {e:#}"));
    let state = parts.kernel_state();

    assert!(
        parts.secrets.encryption_master_key().is_err(),
        "a child host never holds the master key, even from a planted variable"
    );
    assert!(state.find::<ConferralsWithheld>().is_some());
    assert!(
        state.find::<Cipher>().is_none(),
        "the declared value is withheld"
    );
    assert_eq!(
        state.find::<Observed>().cloned(),
        Some(Observed(
            "declared",
            Some(ConferralError::Withheld(InstallConferral::MemberCipher))
        ))
    );
    let undeclared = state.find::<Vec<Observed>>().expect("provided");
    assert!(
        matches!(undeclared[0].1, Some(ConferralError::Missing(_))),
        "{undeclared:?}"
    );
    assert!(
        state
            .find::<DynConferralIssuer<dyn MemberCipher>>()
            .is_none()
    );
    assert!(
        state
            .find::<DynConferralIssuer<dyn ShareTokenSigner>>()
            .is_none()
    );
    assert!(
        state
            .find::<DynConferralIssuer<dyn ScopedTokenMinter>>()
            .is_none()
    );
    assert!(
        state
            .find::<DynConferralIssuer<dyn GitSourceSync>>()
            .is_none()
    );
}
