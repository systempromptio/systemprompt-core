//! The API application the binary builds, over the reference composition:
//! with every member admitted it resolves and mounts, and with each member
//! the kernel allows to be disabled turned off in the profile (together with
//! the members that depend on it) it still resolves and mounts. A member the
//! kernel requires is refused at admission with `RequiredDisabled`.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::WithheldBecause;
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_boot_profile,
};

fn admit(disabled: &[ExtensionId]) -> Result<systemprompt_extension::Admitted, CompositionError> {
    let mut profile = (*test_boot_profile()).clone();
    profile.extensions.disabled = disabled
        .iter()
        .cloned()
        .map(systemprompt_manifest::DisabledExtension::literal)
        .collect();
    reference().build(&profile)
}

fn with_dependants(member: &str) -> Result<Vec<ExtensionId>, CompositionError> {
    let mut disabled = vec![ExtensionId::new(member)];
    loop {
        match admit(&disabled) {
            Err(CompositionError::DisabledDependency { owner, .. })
                if !disabled.contains(&owner) =>
            {
                disabled.push(owner);
            },
            Err(error) => return Err(error),
            Ok(_) => return Ok(disabled),
        }
    }
}

async fn mount(disabled: &[ExtensionId]) -> Result<(), String> {
    let admitted = admit(disabled).map_err(|e| format!("admission: {e}"))?;
    let bootstrap = ensure_test_bootstrap();
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        Arc::new(admitted),
    )
    .await
    .map_err(|e| format!("build: {e:#}"))?;
    setup_api_server(
        &ctx,
        Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map(|_| ())
    .map_err(|e| format!("mount: {e:#}"))
}

#[tokio::test]
async fn the_reference_composition_resolves_and_mounts_with_every_member_admitted() {
    if let Err(error) = mount(&[]).await {
        panic!("the reference composition must build the API application: {error}");
    }
}

#[tokio::test]
async fn each_disableable_member_disabled_in_turn_still_builds_the_api_application() {
    let mut mounted = 0usize;
    let mut failed = Vec::new();
    for id in reference().member_ids() {
        let line = match with_dependants(id) {
            Err(CompositionError::RequiredDisabled(_)) => format!("{id}: required"),
            Err(error) => {
                let line = format!("{id}: refused at admission: {error}");
                failed.push(line.clone());
                line
            },
            Ok(disabled) => match mount(&disabled).await {
                Ok(()) => {
                    mounted += 1;
                    format!("{id}: mounted with {disabled:?} disabled")
                },
                Err(error) => {
                    let line = format!("{id}: failed with {disabled:?} disabled: {error}");
                    failed.push(line.clone());
                    line
                },
            },
        };
        eprintln!("{line}");
    }
    assert!(mounted > 0, "at least one optional member is exercised");
    assert!(failed.is_empty(), "{failed:#?}");
}

#[tokio::test]
async fn oauth_disabled_withholds_identity_http_and_the_api_still_mounts() {
    let oauth = ExtensionId::new("oauth");
    let admitted = admit(std::slice::from_ref(&oauth)).expect("oauth is optional");
    let withheld = admitted.withheld();
    assert!(
        withheld
            .iter()
            .any(|member| member.owner.as_str() == "identity-http"
                && member.dependency == oauth
                && member.because == WithheldBecause::DependencyDisabled),
        "identity-http is withheld with oauth: {withheld:?}"
    );
    assert!(
        !admitted
            .built()
            .contains(&ExtensionId::new("identity-http"))
    );
    if let Err(error) = mount(&[oauth]).await {
        panic!("the API application builds with oauth disabled: {error}");
    }
}

#[test]
fn oauth_removed_from_the_composition_withholds_identity_http() {
    let admitted = reference()
        .without::<systemprompt::oauth::OauthExtension>()
        .build(&test_boot_profile())
        .expect("a composition without oauth admits");
    assert!(
        admitted
            .withheld()
            .iter()
            .any(|member| member.owner.as_str() == "identity-http"
                && member.because == WithheldBecause::DependencyRemoved),
        "{:?}",
        admitted.withheld()
    );
}

#[test]
fn oauth_enabled_withholds_nothing() {
    let admitted = admit(&[]).expect("the reference composition admits");
    assert!(admitted.withheld().is_empty(), "{:?}", admitted.withheld());
    assert!(
        admitted
            .built()
            .contains(&ExtensionId::new("identity-http"))
    );
}
