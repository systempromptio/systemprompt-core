use std::collections::BTreeSet;

use systemprompt_bridge::install::mdm::clear_plan::{
    ClearPlan, ObservedPolicy, is_bridge_loopback, plan_clear,
};

fn observed(keys: &[&str], fp: Option<&str>) -> ObservedPolicy {
    ObservedPolicy {
        keys: keys
            .iter()
            .map(|k| (*k).to_owned())
            .collect::<BTreeSet<_>>(),
        api_key_fingerprint: fp.map(str::to_owned),
        base_url: None,
        profile_managed: false,
    }
}

fn routed_to(url: &str, fp: &str, profile_managed: bool) -> ObservedPolicy {
    ObservedPolicy {
        base_url: Some(url.to_owned()),
        profile_managed,
        ..observed(
            &[
                "inferenceProvider",
                "inferenceGatewayBaseUrl",
                "inferenceGatewayApiKey",
            ],
            Some(fp),
        )
    }
}

#[test]
fn nothing_written_is_a_no_op_whether_or_not_desktop_is_installed() {
    assert_eq!(plan_clear(None, Some("ours")), ClearPlan::NothingWritten);
    assert_eq!(
        plan_clear(Some(&observed(&[], None)), Some("ours")),
        ClearPlan::NothingWritten
    );
    assert!(!ClearPlan::NothingWritten.writes());
}

#[test]
fn the_bridges_own_policy_is_removed_not_rewritten() {
    let plan = plan_clear(
        Some(&observed(
            &[
                "inferenceProvider",
                "inferenceGatewayBaseUrl",
                "inferenceGatewayApiKey",
                "inferenceModels",
                "managedMcpServers",
            ],
            Some("ours"),
        )),
        Some("ours"),
    );
    assert_eq!(
        plan,
        ClearPlan::RemoveAll,
        "a disabled host must leave the gateway"
    );
}

#[test]
fn keys_the_bridge_did_not_write_survive_a_clear() {
    let plan = plan_clear(
        Some(&observed(
            &[
                "inferenceProvider",
                "inferenceGatewayApiKey",
                "someAdminSetting",
            ],
            Some("ours"),
        )),
        Some("ours"),
    );
    match plan {
        ClearPlan::RemoveKeys(keys) => {
            assert!(keys.contains(&"inferenceProvider"));
            assert!(!keys.contains(&"someAdminSetting"));
        },
        other => panic!("expected RemoveKeys, got {other:?}"),
    }
}

#[test]
fn a_policy_carrying_another_secret_or_none_is_managed_elsewhere() {
    let mdm = observed(
        &["inferenceProvider", "inferenceGatewayApiKey"],
        Some("theirs"),
    );
    assert_eq!(
        plan_clear(Some(&mdm), Some("ours")),
        ClearPlan::ManagedElsewhere
    );
    let keyless = observed(&["inferenceProvider"], None);
    assert_eq!(
        plan_clear(Some(&keyless), Some("ours")),
        ClearPlan::ManagedElsewhere
    );
    assert_eq!(
        plan_clear(Some(&mdm), None),
        ClearPlan::ManagedElsewhere,
        "without a loopback secret nothing can be proven ours"
    );
}

#[test]
fn a_previous_bridge_installs_policy_is_stale_not_managed_elsewhere() {
    for url in [
        "http://127.0.0.1:48217",
        "http://localhost:48226/",
        "http://[::1]:48220",
    ] {
        let plan = plan_clear(Some(&routed_to(url, "old-secret", false)), Some("ours"));
        assert_eq!(plan, ClearPlan::StaleBridgePolicy, "{url}");
        assert!(!plan.writes(), "an unattended clear never removes it");
    }
}

#[test]
fn a_policy_for_another_endpoint_or_delivered_by_a_profile_stays_managed_elsewhere() {
    for url in [
        "https://gateway.example.com",
        "http://127.0.0.1:8080",
        "http://127.0.0.1:48227",
        "http://10.0.0.5:48217",
    ] {
        assert_eq!(
            plan_clear(Some(&routed_to(url, "old-secret", false)), Some("ours")),
            ClearPlan::ManagedElsewhere,
            "{url}"
        );
    }
    assert_eq!(
        plan_clear(
            Some(&routed_to("http://127.0.0.1:48217", "old-secret", true)),
            Some("ours")
        ),
        ClearPlan::ManagedElsewhere,
        "a configuration profile from device management is never taken over"
    );
}

#[test]
fn our_own_loopback_policy_is_still_removed() {
    assert_eq!(
        plan_clear(
            Some(&routed_to("http://127.0.0.1:48217", "ours", false)),
            Some("ours")
        ),
        ClearPlan::RemoveAll
    );
    assert!(is_bridge_loopback("http://127.0.0.1:48217"));
    assert!(!is_bridge_loopback("not a url"));
}
