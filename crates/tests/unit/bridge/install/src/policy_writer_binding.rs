//! The machine policy is bound to the installing account: a request owned by
//! any other account, a request that tries to carry its own inference facts,
//! and a replayed or skewed envelope are all refused before anything reaches
//! the machine hive.

use std::collections::BTreeMap;

use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::install::policy_writer::binding::{
    Admission, ApplyOutcome, BINDING_VERSION, ENVELOPE_SKEW_MINUTES, admit, authorize,
    envelope_digest, require_bound_loopback,
};
use systemprompt_bridge::install::policy_writer::{
    AcceptedEnvelope, AccountSid, Loopback, MachineBinding, PolicyWriteRequest, PolicyWriterError,
    derive_policy,
};
use systemprompt_models::bridge::host::HostKind;

use crate::policy_writer::{
    INSTALLER, binding, envelope, manifest, request, server, signing_key, value_of, wildcard,
};

const OTHER_ACCOUNT: &str = "S-1-5-21-1-2-3-1002";

fn sid(raw: &str) -> AccountSid {
    AccountSid::try_new(raw).unwrap()
}

fn digest_of(seed: u8) -> systemprompt_bridge::ids::Sha256Digest {
    envelope_digest(&envelope(&signing_key(seed), &manifest(vec![]))).unwrap()
}

fn accepted(version: &str) -> AcceptedEnvelope {
    AcceptedEnvelope {
        manifest_version: ManifestVersion::try_new(version).unwrap(),
        envelope_digest: digest_of(9),
    }
}

fn accepted_with(
    version: &str,
    digest: systemprompt_bridge::ids::Sha256Digest,
) -> AcceptedEnvelope {
    AcceptedEnvelope {
        manifest_version: ManifestVersion::try_new(version).unwrap(),
        envelope_digest: digest,
    }
}

#[test]
fn a_request_owned_by_another_account_never_reaches_the_machine_policy() {
    let refused = authorize(&sid(OTHER_ACCOUNT), &binding());
    match refused {
        Err(PolicyWriterError::NotBoundInstaller { owner, bound }) => {
            assert_eq!(owner.as_str(), OTHER_ACCOUNT);
            assert_eq!(bound.as_str(), INSTALLER);
        },
        other => panic!("another account's request must be refused, got {other:?}"),
    }
    authorize(&sid(INSTALLER), &binding()).expect("the installing account is admitted");
}

#[test]
fn a_request_cannot_carry_its_own_port_token_headers_models_or_organisation() {
    let key = signing_key(1);
    let req = request(&key, &manifest(vec![]), &[]);
    let json = serde_json::to_value(&req).unwrap();
    for (field, value) in [
        ("loopback_port", serde_json::json!(9000)),
        ("host_token", serde_json::json!("attacker-token")),
        ("headers", serde_json::json!({"x-forwarded-to": "attacker"})),
        ("models", serde_json::json!("[\"attacker-model\"]")),
        (
            "org_uuid",
            serde_json::json!("11111111-2222-4333-8444-555555555555"),
        ),
        ("requester_sid", serde_json::json!(INSTALLER)),
    ] {
        let mut smuggled = json.clone();
        smuggled[field] = value;
        assert!(
            serde_json::from_value::<PolicyWriteRequest>(smuggled).is_err(),
            "a request naming {field} is refused, not obeyed"
        );
    }
}

#[test]
fn the_inference_facts_come_from_the_binding_and_the_signed_manifest() {
    let key = signing_key(1);
    let mut signed = manifest(vec![server(
        "atlassian",
        wildcard(systemprompt_bridge::ids::ToolPolicy::Allow),
    )]);
    signed.host_model_protocols.insert(
        HostKind::ClaudeDesktop,
        vec!["claude".to_owned(), "openai".to_owned()],
    );
    let req = request(&key, &signed, &[("atlassian", &["getJiraIssue"])]);
    let values = derive_policy(
        &binding(),
        &req,
        &signed,
        Some(r#"["claude-opus-5"]"#.to_owned()),
    )
    .unwrap();
    assert_eq!(
        value_of(&values, "inferenceGatewayBaseUrl"),
        Some("http://127.0.0.1:48217")
    );
    assert_eq!(
        value_of(&values, "inferenceGatewayApiKey"),
        Some("host-token-for-desktop")
    );
    let headers: BTreeMap<String, String> =
        serde_json::from_str(value_of(&values, "inferenceCustomHeaders").unwrap()).unwrap();
    assert_eq!(
        headers,
        BTreeMap::from([(
            "x-inference-protocol".to_owned(),
            "claude,openai".to_owned()
        )]),
        "the headers are the signed per-host protocol filter, nothing else"
    );
    assert_eq!(
        value_of(&values, "inferenceModels"),
        Some(r#"["claude-opus-5"]"#)
    );
    assert_eq!(
        value_of(&values, "deploymentOrganizationUuid"),
        None,
        "the writer never writes an organisation; the installed one stays"
    );
    let servers = value_of(&values, "managedMcpServers").unwrap();
    assert!(servers.contains("http://127.0.0.1:48217/mcp/atlassian"));
}

#[test]
fn an_envelope_no_newer_than_the_last_accepted_is_refused() {
    let signed = manifest(vec![]);
    let now = signed.not_before;
    let same = accepted("2026-09-19T08:00:00Z-cafecafe");
    assert!(matches!(
        admit(&signed, &digest_of(1), Some(&same), now),
        Err(PolicyWriterError::Replayed { .. })
    ));
    let newer = accepted("2026-09-20T08:00:00Z-cafecafe");
    assert!(
        matches!(
            admit(&signed, &digest_of(1), Some(&newer), now),
            Err(PolicyWriterError::Replayed { .. })
        ),
        "an older envelope cannot roll the machine policy back"
    );
    let older = accepted("2026-09-18T08:00:00Z-cafecafe");
    assert_eq!(
        admit(&signed, &digest_of(1), Some(&older), now).unwrap(),
        Admission::Fresh
    );
    assert_eq!(
        admit(&signed, &digest_of(1), None, now).unwrap(),
        Admission::Fresh
    );
}

#[test]
fn an_envelope_outside_the_clock_skew_window_is_refused() {
    let signed = manifest(vec![]);
    let beyond = chrono::Duration::minutes(ENVELOPE_SKEW_MINUTES + 1);
    for now in [signed.not_before + beyond, signed.not_before - beyond] {
        assert!(
            matches!(
                admit(&signed, &digest_of(1), None, now),
                Err(PolicyWriterError::Skewed { .. })
            ),
            "not_before {} against now {now} is outside the window",
            signed.not_before
        );
    }
    let within = chrono::Duration::minutes(ENVELOPE_SKEW_MINUTES - 1);
    admit(&signed, &digest_of(1), None, signed.not_before + within).expect("inside the window");
}

#[test]
fn a_bridge_whose_loopback_is_not_the_bound_one_is_refused() {
    let moved = Loopback {
        port: 9000,
        host_token: HostToken::new("host-token-for-desktop"),
    };
    assert!(matches!(
        require_bound_loopback(&binding(), &moved),
        Err(PolicyWriterError::BindingStale {
            bound_port: 48217,
            current_port: 9000
        })
    ));
    let rekeyed = Loopback {
        port: 48217,
        host_token: HostToken::new("another-secret"),
    };
    assert!(require_bound_loopback(&binding(), &rekeyed).is_err());
    require_bound_loopback(&binding(), &binding().loopback()).expect("the bound loopback");
}

#[test]
fn a_binding_decodes_only_in_its_own_shape() {
    let bytes = serde_json::to_vec(&binding()).unwrap();
    assert_eq!(MachineBinding::decode(&bytes).unwrap(), binding());
    let mut wrong_version = serde_json::to_value(binding()).unwrap();
    wrong_version["version"] = serde_json::json!(BINDING_VERSION + 1);
    assert!(matches!(
        MachineBinding::decode(&serde_json::to_vec(&wrong_version).unwrap()),
        Err(PolicyWriterError::BindingVersion { .. })
    ));
    let mut bad_sid = serde_json::to_value(binding()).unwrap();
    bad_sid["installer_sid"] = serde_json::json!("Everyone");
    assert!(MachineBinding::decode(&serde_json::to_vec(&bad_sid).unwrap()).is_err());
}

#[test]
fn only_a_well_formed_sid_is_an_account() {
    for good in [INSTALLER, "S-1-5-18"] {
        assert!(AccountSid::try_new(good).is_ok(), "{good}");
    }
    for bad in ["", "S-1-", "S-1-5-", "S-1-5-x", "WD", "S-1-5--18"] {
        assert!(
            matches!(
                AccountSid::try_new(bad),
                Err(PolicyWriterError::InvalidSid(_))
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn the_envelope_last_accepted_presented_again_is_a_reapply_even_after_the_window() {
    let signed = manifest(vec![]);
    let digest = digest_of(1);
    let last = accepted_with("2026-09-19T08:00:00Z-cafecafe", digest.clone());
    let much_later = signed.not_before + chrono::Duration::days(3);
    assert_eq!(
        admit(&signed, &digest, Some(&last), much_later).unwrap(),
        Admission::AlreadyAccepted,
        "the accepted envelope derives the policy it already derived"
    );
}

#[test]
fn the_same_version_in_a_different_envelope_is_a_replay() {
    let signed = manifest(vec![]);
    let last = accepted_with("2026-09-19T08:00:00Z-cafecafe", digest_of(1));
    assert!(matches!(
        admit(&signed, &digest_of(2), Some(&last), signed.not_before),
        Err(PolicyWriterError::Replayed { .. })
    ));
}

#[test]
fn an_older_envelope_with_the_accepted_digest_is_still_a_replay() {
    let signed = manifest(vec![]);
    let digest = digest_of(1);
    let last = accepted_with("2026-09-20T08:00:00Z-cafecafe", digest.clone());
    assert!(matches!(
        admit(&signed, &digest, Some(&last), signed.not_before),
        Err(PolicyWriterError::Replayed { .. })
    ));
}

#[test]
fn the_digest_names_one_exact_envelope() {
    let key = signing_key(1);
    let signed = manifest(vec![]);
    let first = envelope(&key, &signed);
    assert_eq!(
        envelope_digest(&first).unwrap(),
        envelope_digest(&first.clone()).unwrap()
    );
    let mut edited = first.clone();
    edited.payload.push(' ');
    assert_ne!(
        envelope_digest(&first).unwrap(),
        envelope_digest(&edited).unwrap()
    );
    let resigned = envelope(&signing_key(2), &signed);
    assert_ne!(
        envelope_digest(&first).unwrap(),
        envelope_digest(&resigned).unwrap()
    );
}

#[test]
fn a_reapply_reports_whether_the_hive_had_drifted() {
    assert_eq!(
        ApplyOutcome::of(Admission::Fresh, false),
        ApplyOutcome::Written
    );
    assert_eq!(
        ApplyOutcome::of(Admission::Fresh, true),
        ApplyOutcome::Written
    );
    assert_eq!(
        ApplyOutcome::of(Admission::AlreadyAccepted, true),
        ApplyOutcome::Reapplied
    );
    assert_eq!(
        ApplyOutcome::of(Admission::AlreadyAccepted, false),
        ApplyOutcome::Unchanged
    );
}
