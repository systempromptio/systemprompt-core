//! The API application the binary builds, over the reference composition:
//! with every member admitted it resolves and mounts, and with each member
//! the kernel allows to be disabled turned off in the profile (together with
//! the members that depend on it) it still resolves and mounts. A member the
//! kernel requires is refused at admission with `RequiredDisabled`.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::AbsentDependency;
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
            Err(
                CompositionError::DisabledDependency { owner, .. }
                | CompositionError::WithheldDependency { owner, .. },
            ) if !disabled.contains(&owner) => {
                disabled.push(owner);
            },
            Err(CompositionError::BearerRoutesWithoutOauth { members })
                if members.iter().any(|member| !disabled.contains(member)) =>
            {
                let fresh: Vec<ExtensionId> = members
                    .into_iter()
                    .filter(|member| !disabled.contains(member))
                    .collect();
                disabled.extend(fresh);
            },
            Err(error) => return Err(error),
            Ok(_) => return Ok(disabled),
        }
    }
}

async fn mount(disabled: &[ExtensionId]) -> Result<(), String> {
    mount_admitted(admit(disabled).map_err(|e| format!("admission: {e}"))?).await
}

async fn mount_admitted(admitted: systemprompt_extension::Admitted) -> Result<(), String> {
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

#[test]
fn oauth_disabled_with_bearer_routes_is_refused_naming_their_members() {
    let error =
        admit(&[ExtensionId::new("oauth")]).expect_err("bearer-authenticated routes need oauth");
    let CompositionError::BearerRoutesWithoutOauth { members } = &error else {
        panic!("expected the oauth refusal, got {error:?}");
    };
    assert!(
        members
            .iter()
            .any(|member| member.as_str() == "content-http"),
        "{members:?}"
    );
    let error = reference()
        .without::<systemprompt::oauth::OauthExtension>()
        .build(&test_boot_profile())
        .expect_err("removing oauth is refused the same way");
    assert!(
        matches!(error, CompositionError::BearerRoutesWithoutOauth { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn oauth_disabled_without_bearer_routes_admits_and_mounts() {
    let disabled = with_dependants("oauth").expect("a composition without bearer routes admits");
    let admitted = admit(&disabled).expect("oauth and the bearer members disabled admit");
    let oauth = ExtensionId::new("oauth");
    assert!(
        admitted
            .withheld()
            .iter()
            .any(|member| member.owner.as_str() == "identity-http"
                && member.dependency == oauth
                && member.because == AbsentDependency::Disabled),
        "identity-http is withheld with oauth: {:?}",
        admitted.withheld()
    );
    assert!(
        admitted
            .routes()
            .iter()
            .all(|set| set.set.defs().iter().all(|def| def
                .spec
                .policy
                .handler_verifier()
                .is_some()
                || def
                    .spec
                    .policy
                    .permits(systemprompt_models::auth::UserType::Anon))),
        "no bearer-authenticated route is admitted without oauth"
    );
    if let Err(error) = mount(&disabled).await {
        panic!("the API application builds with oauth disabled: {error}");
    }
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

struct SiteClaimant;

impl systemprompt_extension::Extension for SiteClaimant {
    fn metadata(&self) -> systemprompt_extension::ExtensionMetadata {
        systemprompt_extension::ExtensionMetadata {
            id: "probe-site",
            name: "probe-site",
            version: "0.0.0",
        }
    }

    fn contribute(
        &self,
        c: &mut systemprompt_extension::Contributions<'_>,
    ) -> Result<(), systemprompt_extension::ContributionError> {
        c.site_fallback(systemprompt_extension::router::SiteFallback::service(
            systemprompt::prelude::Router::new(),
        ));
        Ok(())
    }
}

#[tokio::test]
async fn a_claimed_site_fallback_boots_through_the_real_mount_beside_static_content() {
    let admitted = reference()
        .with(SiteClaimant)
        .build(&test_boot_profile())
        .expect("one site fallback claim admits");
    assert!(admitted.site_fallback().is_some());
    if let Err(error) = mount_admitted(admitted).await {
        panic!("the API application builds with a site fallback claimed: {error}");
    }
}
