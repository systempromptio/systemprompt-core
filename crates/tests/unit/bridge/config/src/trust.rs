use systemprompt_bridge::config::trust::{GatewayIdentity, pinned_pubkey_state_for};
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
