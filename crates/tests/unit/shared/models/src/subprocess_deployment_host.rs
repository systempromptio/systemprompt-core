//! The decision table behind "am I already on the host my cloud profile
//! describes?".
//!
//! `deployment_host` is the seam the CLI routing layer answers from: `false`
//! means the command is forwarded to a remote tenant, `true` means it runs
//! here. Both functions take an injected lookup, so every row below is driven
//! without touching the process environment.

use systemprompt_models::subprocess::{
    ANY_DEPLOYMENT_HOST_MARKERS, DEPLOYMENT_HOST_ENV, inherited_parent_env,
};

fn deployment_host(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    systemprompt_models::subprocess::deployment_host(ANY_DEPLOYMENT_HOST_MARKERS, lookup)
}

fn is_deployment_host(lookup: impl Fn(&str) -> Option<String>) -> bool {
    systemprompt_models::subprocess::is_deployment_host(ANY_DEPLOYMENT_HOST_MARKERS, lookup)
}

fn nothing_set(_: &str) -> Option<String> {
    None
}

fn only(name: &'static str, value: &'static str) -> impl Fn(&str) -> Option<String> {
    move |asked: &str| (asked == name).then(|| value.to_owned())
}

#[test]
fn the_generated_marker_names_the_host() {
    assert_eq!(
        deployment_host(only(DEPLOYMENT_HOST_ENV, "sp-tenant")),
        Some("sp-tenant".to_owned())
    );
    assert!(is_deployment_host(only(DEPLOYMENT_HOST_ENV, "sp-tenant")));
}

// Why: tenants deployed before the generated marker existed carry only Fly's,
// and must keep running locally rather than trying to route to themselves.
#[test]
fn flys_marker_alone_still_proves_we_are_on_the_host() {
    assert_eq!(
        deployment_host(only("FLY_APP_NAME", "sp-legacy")),
        Some("sp-legacy".to_owned())
    );
    assert!(is_deployment_host(only("FLY_APP_NAME", "sp-legacy")));
}

// Why: the marker we generate is the one an operator can steer, so it has to
// win when both are present — otherwise an override is silently ignored.
#[test]
fn the_generated_marker_wins_over_flys_when_both_are_present() {
    let both = |name: &str| match name {
        DEPLOYMENT_HOST_ENV => Some("sp-chosen".to_owned()),
        "FLY_APP_NAME" => Some("sp-fly".to_owned()),
        _ => None,
    };

    assert_eq!(deployment_host(both), Some("sp-chosen".to_owned()));
}

// Why: an exported-but-empty variable is how a shell leaves a marker behind.
// Counting it as present would tell an operator's laptop it is the tenant and
// run every cloud command against the local database.
#[test]
fn an_empty_marker_counts_as_absent() {
    assert_eq!(deployment_host(only(DEPLOYMENT_HOST_ENV, "")), None);
    assert!(!is_deployment_host(only(DEPLOYMENT_HOST_ENV, "")));
}

#[test]
fn a_whitespace_only_marker_counts_as_absent() {
    assert_eq!(deployment_host(only(DEPLOYMENT_HOST_ENV, "   ")), None);
    assert!(!is_deployment_host(only("FLY_APP_NAME", "\t\n")));
}

#[test]
fn a_padded_marker_is_trimmed_to_the_host_name() {
    assert_eq!(
        deployment_host(only(DEPLOYMENT_HOST_ENV, "  sp-tenant \n")),
        Some("sp-tenant".to_owned())
    );
}

// Why: an empty generated marker must not mask a real Fly one; the empty value
// is skipped and the fallback still answers.
#[test]
fn an_empty_generated_marker_falls_through_to_flys() {
    let mixed = |name: &str| match name {
        DEPLOYMENT_HOST_ENV => Some(String::new()),
        "FLY_APP_NAME" => Some("sp-fly".to_owned()),
        _ => None,
    };

    assert_eq!(deployment_host(mixed), Some("sp-fly".to_owned()));
    assert!(is_deployment_host(mixed));
}

#[test]
fn neither_marker_means_we_are_elsewhere() {
    assert_eq!(deployment_host(nothing_set), None);
    assert!(!is_deployment_host(nothing_set));
}

// Why: only these two names may answer the question. A neighbouring variable
// that happens to name a host must not be mistaken for the marker.
#[test]
fn an_unrelated_variable_never_answers_the_question() {
    assert_eq!(deployment_host(only("FLY_REGION", "lhr")), None);
    assert!(!is_deployment_host(only("HOSTNAME", "sp-tenant")));
}

#[test]
fn the_inherited_set_is_exactly_the_allowlist_that_is_set() {
    let parent = |name: &str| match name {
        DEPLOYMENT_HOST_ENV => Some("sp-tenant".to_owned()),
        "FLY_APP_NAME" => Some("sp-fly".to_owned()),
        "PATH" => Some("/usr/bin".to_owned()),
        "HOME" => Some("/home/sp".to_owned()),
        "HOSTNAME" => Some("node-a".to_owned()),
        "POD_NAME" => Some("sp-api-7c9f".to_owned()),
        "FLY_MACHINE_ID" => Some("0801e1a2".to_owned()),
        _ => None,
    };

    let env = inherited_parent_env(parent);

    assert_eq!(
        env,
        vec![
            (DEPLOYMENT_HOST_ENV.to_owned(), "sp-tenant".to_owned()),
            ("FLY_APP_NAME".to_owned(), "sp-fly".to_owned()),
            ("HOSTNAME".to_owned(), "node-a".to_owned()),
            ("POD_NAME".to_owned(), "sp-api-7c9f".to_owned()),
            ("FLY_MACHINE_ID".to_owned(), "0801e1a2".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
            ("HOME".to_owned(), "/home/sp".to_owned()),
        ]
    );
}

#[test]
fn nothing_is_inherited_when_the_parent_has_none_of_it() {
    let env = inherited_parent_env(nothing_set);

    assert!(
        env.is_empty(),
        "an absent variable must be omitted, not forwarded as an empty string: {env:?}"
    );
}

// Why: the trust allowlist rides along with the same set, and unlike the host
// markers it is forwarded verbatim — a child that loses it re-derives a
// different outbound policy from its parent.
#[test]
fn the_trusted_hosts_allowlist_rides_along_with_the_inherited_set() {
    let env = inherited_parent_env(only(
        "SYSTEMPROMPT_TRUSTED_HTTP_HOSTS",
        "a.example,b.example",
    ));

    assert_eq!(
        env,
        vec![(
            "SYSTEMPROMPT_TRUSTED_HTTP_HOSTS".to_owned(),
            "a.example,b.example".to_owned()
        )]
    );
}

#[test]
fn a_marker_outside_the_given_set_is_not_consulted() {
    let fly_only = only("FLY_APP_NAME", "sp-fly");
    assert!(!systemprompt_models::subprocess::is_deployment_host(
        &[DEPLOYMENT_HOST_ENV],
        fly_only
    ));
}

#[test]
fn the_systemprompt_provider_still_honours_flys_marker() {
    use systemprompt_manifest::HostingProvider;
    assert!(HostingProvider::Systemprompt.is_deployment_host(only("FLY_APP_NAME", "sp-fly")));
}

// Why: a Kubernetes pod that happens to carry FLY_APP_NAME (a copied env
// block, a migration script) must not be mistaken for a Fly machine.
#[test]
fn a_kubernetes_provider_ignores_flys_marker_and_reads_the_generated_one() {
    use systemprompt_manifest::HostingProvider;
    assert!(!HostingProvider::Kubernetes.is_deployment_host(only("FLY_APP_NAME", "sp-fly")));
    assert!(HostingProvider::Kubernetes.is_deployment_host(only(DEPLOYMENT_HOST_ENV, "acme")));
}

#[test]
fn a_profile_without_a_provider_is_the_systemprompt_cloud() {
    let cloud: systemprompt_manifest::CloudConfig =
        serde_yaml::from_str("validation: strict").unwrap();
    assert_eq!(
        cloud.provider,
        systemprompt_manifest::HostingProvider::Systemprompt
    );
}

#[test]
fn providers_parse_from_snake_case() {
    use systemprompt_manifest::HostingProvider;
    for provider in HostingProvider::ALL {
        assert_eq!(
            provider.as_str().parse::<HostingProvider>().unwrap(),
            provider
        );
    }
    assert!("fly".parse::<HostingProvider>().is_err());
}
