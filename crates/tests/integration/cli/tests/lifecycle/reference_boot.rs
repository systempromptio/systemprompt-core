//! The API application the binary builds, over the reference composition:
//! with every member admitted it resolves and mounts, and with each member
//! the kernel allows to be disabled turned off in the profile (together with
//! the members that depend on it) it still resolves and mounts. A member the
//! kernel requires is refused at admission with `RequiredDisabled`.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_boot_profile,
};

#[derive(Debug)]
enum Outcome {
    Mounted { disabled: Vec<ExtensionId> },
    Required,
    Failed { disabled: Vec<ExtensionId>, error: String },
}

fn admit(disabled: &[ExtensionId]) -> Result<systemprompt_extension::Admitted, CompositionError> {
    let mut profile = (*test_boot_profile()).clone();
    profile.extensions.disabled = disabled.to_vec();
    reference().build(&profile)
}

fn with_dependants(member: &str) -> Result<Vec<ExtensionId>, CompositionError> {
    let mut disabled = vec![ExtensionId::new(member)];
    loop {
        match admit(&disabled) {
            Err(CompositionError::DisabledDependency { owner, .. }) if !disabled.contains(&owner) => {
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
    let mut outcomes = Vec::new();
    for id in reference().member_ids() {
        let outcome = match with_dependants(id) {
            Err(CompositionError::RequiredDisabled(_)) => Outcome::Required,
            Err(error) => Outcome::Failed {
                disabled: vec![ExtensionId::new(id)],
                error: format!("admission: {error}"),
            },
            Ok(disabled) => match mount(&disabled).await {
                Ok(()) => Outcome::Mounted { disabled },
                Err(error) => Outcome::Failed { disabled, error },
            },
        };
        eprintln!("{id}: {outcome:?}");
        outcomes.push((id, outcome));
    }
    assert!(
        outcomes
            .iter()
            .any(|(_, outcome)| matches!(outcome, Outcome::Mounted { .. })),
        "at least one optional member is exercised: {outcomes:?}"
    );
    let failed: Vec<_> = outcomes
        .iter()
        .filter(|(_, outcome)| matches!(outcome, Outcome::Failed { .. }))
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}
