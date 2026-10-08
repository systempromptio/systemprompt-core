use systemprompt_bridge::config::trust::{
    GatewayIdentity, PinSource, TrustError, TrustRecord, parse_policy_trust,
    resolve_pinned_pubkey_state,
};
use systemprompt_bridge::config::{Config, PinnedPubkeyState};
use systemprompt_identifiers::ValidatedUrl;

#[test]
fn gateway_identity_preserves_tenant_path_and_port() {
    let a = GatewayIdentity::try_from("https://EXAMPLE.com:443/tenant/a/".to_owned()).unwrap();
    assert_eq!(a.as_str(), "https://example.com/tenant/a");
    assert_ne!(
        a,
        GatewayIdentity::try_from("https://example.com/tenant/b".to_owned()).unwrap()
    );
    assert_ne!(
        a,
        GatewayIdentity::try_from("https://example.com:444/tenant/a".to_owned()).unwrap()
    );
    assert!(GatewayIdentity::try_from("https://user:password@example.com".to_owned()).is_err());
}

#[test]
fn a_leftover_pinned_pubkey_key_is_ignored_and_only_the_trust_record_counts() {
    let cfg: Config = toml::from_str(&format!(
        "[sync]\npinned_pubkey = 'ignored'\n[sync.trust]\ngateway = 'https://example.com'\nkey = '{VALID_KEY}'\nsource = 'operator'\n",
    ))
    .unwrap();
    let gateway = ValidatedUrl::try_new("https://example.com").unwrap();
    let state = operator_only(&cfg, &gateway).unwrap();
    assert!(
        matches!(state, PinnedPubkeyState::Pinned { .. }),
        "{state:?}"
    );
}

// `[sync] pinned_pubkey` alone is no longer a trust source: without a bound
// record the bridge is unpinned, never trust-on-first-use.
#[test]
fn a_bare_pinned_pubkey_key_without_a_trust_record_is_unpinned() {
    let cfg: Config = toml::from_str(&format!(
        "[sync]\npinned_pubkey = '{VALID_KEY}'\npinned_pubkey_gateway = 'https://example.com/'",
    ))
    .unwrap();
    let gateway = ValidatedUrl::try_new("https://example.com").unwrap();
    let state = operator_only(&cfg, &gateway).unwrap();
    assert_eq!(state, PinnedPubkeyState::Unpinned, "{state:?}");
}

const VALID_KEY: &str = "WGZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY=";
const MALFORMED_KEY: &str = "not base64 and far too short";
fn operator_trust(gateway: &str, key: &str) -> Config {
    toml::from_str(&format!(
        "[sync.trust]\ngateway = '{gateway}'\nkey = '{key}'\nsource = 'operator'\n"
    ))
    .expect("a trust record parses without validating its key")
}

fn operator_only(cfg: &Config, gateway: &ValidatedUrl) -> Result<PinnedPubkeyState, TrustError> {
    let operator = cfg.sync.as_ref().and_then(|s| s.trust.as_ref());
    resolve_pinned_pubkey_state(None, operator, gateway)
}

fn managed(record: &str, gateway: &ValidatedUrl) -> Result<PinnedPubkeyState, TrustError> {
    let policy = parse_policy_trust(record).expect("the managed record parses");
    resolve_pinned_pubkey_state(Some(&policy), None, gateway)
}

#[test]
fn a_record_for_another_gateway_is_judged_before_its_key_is_validated() {
    // Why: validating the key first turned "pinned for a different gateway"
    // into a base64 decoding error, which names nothing the operator can act
    // on and hides the real reason the pin does not apply.
    let cfg = operator_trust("https://old.example.com", MALFORMED_KEY);
    let gateway = ValidatedUrl::try_new("https://new.example.com").expect("url");
    let state = operator_only(&cfg, &gateway).expect("a stale record is a state, not an error");
    assert_eq!(
        state,
        PinnedPubkeyState::Unpinned,
        "an operator pin is trust learned per gateway; a new gateway is simply unpinned"
    );
}

#[test]
fn an_operator_record_for_the_current_gateway_still_validates_its_key() {
    // Why: the negative control. Skipping validation for a *matching* gateway
    // would let an unusable key be reported as pinned.
    let cfg = operator_trust("https://gw.example.com", MALFORMED_KEY);
    let gateway = ValidatedUrl::try_new("https://gw.example.com").expect("url");
    let err = operator_only(&cfg, &gateway).expect_err("a key that will be used is checked");
    assert!(
        err.to_string().contains("not base64"),
        "the operator is told the key itself is unusable: {err}"
    );

    let usable = operator_trust("https://gw.example.com", VALID_KEY);
    let state = operator_only(&usable, &gateway).expect("valid");
    assert!(
        matches!(state, PinnedPubkeyState::Pinned { ref key, .. } if key.as_str() == VALID_KEY),
        "{state:?}"
    );
}

#[test]
fn a_managed_pin_for_another_gateway_is_reported_stale_rather_than_ignored() {
    // Why: an administrator named a key for one gateway; pointing the bridge
    // at another is a conflict only they can resolve, so it must not silently
    // degrade to unpinned the way an operator pin does.
    let record = format!(
        r#"{{"gateway":"https://managed.example.com","key":"{VALID_KEY}","source":"policy"}}"#
    );
    let gateway = ValidatedUrl::try_new("https://other.example.com").expect("url");
    let state = managed(&record, &gateway).expect("a stale managed pin is a state");
    match state {
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            assert_eq!(pinned_for.as_str(), "https://managed.example.com");
            assert_eq!(current.as_str(), "https://other.example.com");
        },
        other => panic!("a managed pin for another gateway must be reported stale: {other:?}"),
    }
}

fn policy_record(gateway: &str, key: &str) -> String {
    format!(r#"{{"gateway":"{gateway}","key":"{key}","source":"policy"}}"#)
}

#[test]
fn a_stale_managed_pin_is_reported_stale_even_when_its_key_is_unusable() {
    // Why: a managed record for a gateway the bridge no longer talks to is
    // stale whatever its key looks like. Decoding the key first reported
    // "not base64" for a record that was never going to be used, which sent
    // the administrator to fix the wrong thing.
    let record = policy_record("https://managed.example.com", MALFORMED_KEY);
    let gateway = ValidatedUrl::try_new("https://other.example.com").expect("url");
    let state =
        managed(&record, &gateway).expect("a stale managed pin is a state, not a decoding error");
    match state {
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            assert_eq!(pinned_for.as_str(), "https://managed.example.com");
            assert_eq!(current.as_str(), "https://other.example.com");
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_managed_pin_for_the_current_gateway_is_still_refused_when_its_key_is_unusable() {
    // Why: the negative control. Deferring key validation must not skip it —
    // a key that is about to be used to verify a manifest is checked.
    let gateway = ValidatedUrl::try_new("https://gw.example.com").expect("url");

    let record = policy_record("https://gw.example.com", MALFORMED_KEY);
    let err = managed(&record, &gateway).expect_err("a key that will be used is checked");
    assert!(
        matches!(err, TrustError::KeyEncoding(_)),
        "a non-base64 managed key is a decoding failure: {err:?}"
    );

    let short = policy_record("https://gw.example.com", "aGVsbG8=");
    let err = managed(&short, &gateway).expect_err("a short key is checked too");
    assert!(
        matches!(err, TrustError::KeyLength { actual: 5 }),
        "the length failure names the bytes it got: {err:?}"
    );
}

#[test]
fn a_managed_pin_outranks_an_operator_pin_for_the_same_gateway() {
    let gateway = ValidatedUrl::try_new("https://gw.example.com").expect("url");
    let policy = parse_policy_trust(&policy_record("https://gw.example.com", VALID_KEY))
        .expect("managed record");
    let operator_key = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
    let operator =
        TrustRecord::new(&gateway, operator_key, PinSource::Operator).expect("operator record");
    let state = resolve_pinned_pubkey_state(Some(&policy), Some(&operator), &gateway)
        .expect("both records are valid");
    assert_eq!(
        state,
        PinnedPubkeyState::Pinned {
            key: policy.key.clone(),
            source: PinSource::Policy,
        }
    );
}

#[test]
fn a_managed_record_is_labelled_policy_whatever_source_it_claims() {
    let record = r#"{"gateway":"https://gw.example.com","key":"k","source":"operator"}"#;
    let parsed = parse_policy_trust(record).expect("record parses");
    assert_eq!(parsed.source, PinSource::Policy);
}

#[test]
fn a_malformed_managed_record_is_an_invalid_policy_error() {
    assert!(matches!(
        parse_policy_trust("invalid json"),
        Err(TrustError::InvalidPolicy(_))
    ));
}

#[test]
fn the_process_environment_cannot_supply_or_replace_managed_trust() {
    let gateway = ValidatedUrl::try_new("https://gw.example.com").expect("url");
    let attacker = policy_record("https://gw.example.com", VALID_KEY);
    let cfg = operator_trust("https://gw.example.com", MALFORMED_KEY);
    let result = temp_env::with_var("SP_BRIDGE_POLICY_TRUST", Some(attacker.as_str()), || {
        systemprompt_bridge::config::trust::pinned_pubkey_state_for(&cfg, &gateway)
    });
    assert!(
        matches!(result, Err(TrustError::KeyEncoding(_))),
        "the operator record is what is judged; the env record never wins: {result:?}"
    );
}
