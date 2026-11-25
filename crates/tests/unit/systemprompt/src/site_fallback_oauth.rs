//! The composition's one site-fallback slot and the oauth admission rule.
//!
//! One member may claim the site fallback; a second claimant, another member
//! or the same one again, is refused naming both. A composition without the
//! `oauth` member refuses every bearer-authenticated route, naming the
//! members that declare one; with `oauth` composed the same routes admit.

use axum::routing::get;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::router::SiteFallback;

use crate::probe::{Probe, ProbeState, build, ok, profile};

fn claimant(id: &'static str, claims: usize) -> Probe {
    Probe::new(id, move |c| {
        for _ in 0..claims {
            c.site_fallback(SiteFallback::service(get(ok).with_state(())));
        }
    })
}

#[test]
fn one_site_fallback_claim_is_admitted_with_its_owner() {
    let admitted = build(reference().with(claimant("probe-site", 1))).expect("one claimant admits");
    assert_eq!(
        admitted.site_fallback().map(|site| site.owner.as_str()),
        Some("probe-site")
    );
    let none = build(reference()).expect("the reference composition admits");
    assert!(
        none.site_fallback().is_none(),
        "negative control: no claimant, no slot"
    );
}

fn stateful_claimant(id: &'static str) -> Probe {
    Probe::new(id, move |c| {
        c.site_fallback(SiteFallback::with_state(|_: ProbeState| {
            get(ok).with_state(())
        }));
    })
}

#[test]
fn a_fallback_built_from_member_state_is_still_the_one_slot() {
    let admitted =
        build(reference().with(stateful_claimant("probe-stateful"))).expect("one claimant admits");
    assert_eq!(
        admitted.site_fallback().map(|site| site.owner.as_str()),
        Some("probe-stateful")
    );
    let error = build(
        reference()
            .with(stateful_claimant("probe-stateful"))
            .with(claimant("probe-site", 1)),
    )
    .expect_err("a second claimant is refused, typed");
    assert!(
        matches!(
            &error,
            CompositionError::SecondSiteFallback { first, second }
                if first.as_str() == "probe-stateful" && second.as_str() == "probe-site"
        ),
        "{error:?}"
    );
}

#[test]
fn a_second_site_fallback_claimant_is_refused_naming_both() {
    let error = build(
        reference()
            .with(claimant("probe-site-a", 1))
            .with(claimant("probe-site-b", 1)),
    )
    .expect_err("two claimants are refused");
    assert!(
        matches!(
            &error,
            CompositionError::SecondSiteFallback { first, second }
                if first.as_str() == "probe-site-a" && second.as_str() == "probe-site-b"
        ),
        "{error:?}"
    );
    let error = build(reference().with(claimant("probe-site", 2)))
        .expect_err("one member claiming twice is refused");
    assert!(
        matches!(
            &error,
            CompositionError::SecondSiteFallback { first, second }
                if first.as_str() == "probe-site" && second.as_str() == "probe-site"
        ),
        "{error:?}"
    );
}

#[test]
fn oauth_disabled_refuses_the_members_declaring_bearer_routes() {
    let error = reference()
        .build(&profile(&["oauth"]))
        .expect_err("bearer routes without oauth are refused");
    let CompositionError::BearerRoutesWithoutOauth { members } = &error else {
        panic!("expected the oauth refusal, got {error:?}");
    };
    assert!(
        members
            .iter()
            .any(|member| member.as_str() == "content-http"),
        "{members:?}"
    );
    assert!(
        !members.iter().any(|member| member.as_str() == "mcp-http"),
        "a member with public routes only is not named: {members:?}"
    );
    reference()
        .build(&profile(&[]))
        .expect("negative control: with oauth composed the same routes admit");
}

#[test]
fn oauth_disabled_also_names_the_members_holding_gateway_routes() {
    let mut disabled: Vec<String> = vec!["oauth".to_owned()];
    let mut named: Vec<String> = Vec::new();
    let admitted = loop {
        let ids: Vec<&str> = disabled.iter().map(String::as_str).collect();
        match reference().build(&profile(&ids)) {
            Ok(admitted) => break admitted,
            Err(CompositionError::BearerRoutesWithoutOauth { members }) => {
                let fresh: Vec<String> = members
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .filter(|member| !disabled.contains(member))
                    .collect();
                assert!(
                    !fresh.is_empty(),
                    "the refusal names a new member: {members:?}"
                );
                named.extend(fresh.iter().cloned());
                disabled.extend(fresh);
            },
            Err(
                CompositionError::DisabledDependency { owner, .. }
                | CompositionError::WithheldDependency { owner, .. },
            ) => {
                disabled.push(owner.as_str().to_owned());
            },
            Err(error) => panic!("unexpected refusal: {error:?}"),
        }
    };
    assert!(
        named.iter().any(|member| member == "notifications-http"),
        "the bridge-notifications gateway route is bearer-authenticated: {named:?}"
    );
    assert!(
        admitted.gateway_routes().is_empty(),
        "no gateway route is admitted without oauth"
    );
}
