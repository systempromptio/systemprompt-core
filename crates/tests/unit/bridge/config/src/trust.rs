use systemprompt_bridge::config::trust::{GatewayIdentity, TrustError, pinned_pubkey_state_for};
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
fn legacy_pin_requires_explicit_trust_even_when_it_recorded_an_origin() {
    let cfg: Config = toml::from_str(
        "[sync]\npinned_pubkey = 'old'\npinned_pubkey_gateway = 'https://example.com'",
    )
    .unwrap();
    let gateway = ValidatedUrl::try_new("https://example.com").unwrap();
    assert!(matches!(
        pinned_pubkey_state_for(&cfg, &gateway).unwrap(),
        PinnedPubkeyState::StaleForGateway { .. }
    ));
}

const VALID_KEY: &str = "WGZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY=";
const MALFORMED_KEY: &str = "not base64 and far too short";
const POLICY_TRUST_ENV: &str = "SP_BRIDGE_POLICY_TRUST";

fn operator_trust(gateway: &str, key: &str) -> Config {
    toml::from_str(&format!(
        "[sync.trust]\ngateway = '{gateway}'\nkey = '{key}'\nsource = 'operator'\n"
    ))
    .expect("a trust record parses without validating its key")
}

fn without_policy<T>(f: impl FnOnce() -> T) -> T {
    temp_env::with_var(POLICY_TRUST_ENV, None::<&str>, f)
}

#[test]
fn a_record_for_another_gateway_is_judged_before_its_key_is_validated() {
    // Why: validating the key first turned "pinned for a different gateway"
    // into a base64 decoding error, which names nothing the operator can act
    // on and hides the real reason the pin does not apply.
    let cfg = operator_trust("https://old.example.com", MALFORMED_KEY);
    let gateway = ValidatedUrl::try_new("https://new.example.com").expect("url");
    let state = without_policy(|| {
        pinned_pubkey_state_for(&cfg, &gateway).expect("a stale record is a state, not an error")
    });
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
    let err = without_policy(|| {
        pinned_pubkey_state_for(&cfg, &gateway).expect_err("a key that will be used is checked")
    });
    assert!(
        err.to_string().contains("not base64"),
        "the operator is told the key itself is unusable: {err}"
    );

    let usable = operator_trust("https://gw.example.com", VALID_KEY);
    let state = without_policy(|| pinned_pubkey_state_for(&usable, &gateway).expect("valid"));
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
    let cfg = Config::default();
    let gateway = ValidatedUrl::try_new("https://other.example.com").expect("url");
    let state = temp_env::with_var(POLICY_TRUST_ENV, Some(record.as_str()), || {
        pinned_pubkey_state_for(&cfg, &gateway).expect("a stale managed pin is a state")
    });
    match state {
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            assert_eq!(pinned_for, "https://managed.example.com");
            assert_eq!(current, "https://other.example.com");
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
    let cfg = Config::default();
    let gateway = ValidatedUrl::try_new("https://other.example.com").expect("url");
    let state = temp_env::with_var(POLICY_TRUST_ENV, Some(record.as_str()), || {
        pinned_pubkey_state_for(&cfg, &gateway)
            .expect("a stale managed pin is a state, not a decoding error")
    });
    match state {
        PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            assert_eq!(pinned_for, "https://managed.example.com");
            assert_eq!(current, "https://other.example.com");
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_managed_pin_for_the_current_gateway_is_still_refused_when_its_key_is_unusable() {
    // Why: the negative control. Deferring key validation must not skip it —
    // a key that is about to be used to verify a manifest is checked.
    let cfg = Config::default();
    let gateway = ValidatedUrl::try_new("https://gw.example.com").expect("url");

    let record = policy_record("https://gw.example.com", MALFORMED_KEY);
    let err = temp_env::with_var(POLICY_TRUST_ENV, Some(record.as_str()), || {
        pinned_pubkey_state_for(&cfg, &gateway).expect_err("a key that will be used is checked")
    });
    assert!(
        matches!(err, TrustError::KeyEncoding(_)),
        "a non-base64 managed key is a decoding failure: {err:?}"
    );

    let short = policy_record("https://gw.example.com", "aGVsbG8=");
    let err = temp_env::with_var(POLICY_TRUST_ENV, Some(short.as_str()), || {
        pinned_pubkey_state_for(&cfg, &gateway).expect_err("a short key is checked too")
    });
    assert!(
        matches!(err, TrustError::KeyLength { actual: 5 }),
        "the length failure names the bytes it got: {err:?}"
    );
}

#[test]
fn policy_pubkey_validates_the_key_regardless_of_which_gateway_it_names() {
    // Why: `policy_pubkey` hands the key straight to signature verification,
    // so it has no gateway comparison to defer behind and must validate
    // eagerly. Moving validation out of `policy_trust` must not have moved
    // it out of this path too.
    let stale = policy_record("https://managed.example.com", MALFORMED_KEY);
    let err = temp_env::with_var(POLICY_TRUST_ENV, Some(stale.as_str()), || {
        systemprompt_bridge::config::trust::policy_pubkey()
            .expect_err("an unusable managed key is never handed out")
    });
    assert!(matches!(err, TrustError::KeyEncoding(_)), "{err:?}");

    let usable = policy_record("https://managed.example.com", VALID_KEY);
    let key = temp_env::with_var(POLICY_TRUST_ENV, Some(usable.as_str()), || {
        systemprompt_bridge::config::trust::policy_pubkey().expect("a valid managed key is read")
    });
    assert_eq!(
        key.expect("a policy pin is present").as_str(),
        VALID_KEY,
        "the negative control: a valid key still comes back"
    );
}
