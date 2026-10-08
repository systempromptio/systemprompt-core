use systemprompt_bridge::gateway::manifest::{
    ManifestError, SignedManifestBuilder, SignedManifestEnvelope, bridge_version_is_supported,
    decode_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_test_fixtures::fixture_user_id;

fn envelope_requiring(floor: Option<&str>) -> SignedManifestEnvelope {
    let manifest = SignedManifestBuilder::new(
        ManifestVersion::try_new("2026-04-22T00:00:00Z-01abcdef").expect("version"),
        chrono::DateTime::parse_from_rfc3339("2026-04-22T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        chrono::DateTime::parse_from_rfc3339("2026-04-22T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        fixture_user_id(),
    )
    .build();
    let mut value = serde_json::to_value(&manifest).expect("serializes");
    value["min_bridge_version"] = floor.map_or(serde_json::Value::Null, |f| f.into());
    SignedManifestEnvelope {
        payload: value.to_string(),
        signature: ManifestSignature::new(""),
    }
}

#[test]
fn a_floor_above_this_build_is_rejected_with_both_versions_named() {
    let err = decode_payload(&envelope_requiring(Some("999.0.0")))
        .expect_err("a bridge below the floor must not sync");
    match err {
        ManifestError::BridgeTooOld { local, required } => {
            assert_eq!(required, "999.0.0");
            assert!(
                !local.is_empty(),
                "the local version is reported to the user"
            );
        },
        other => panic!("expected BridgeTooOld, got {other:?}"),
    }
}

#[test]
fn a_floor_at_or_below_this_build_is_accepted() {
    decode_payload(&envelope_requiring(Some("0.0.1"))).expect("a supported bridge syncs");
}

#[test]
fn a_gateway_that_declares_no_floor_is_accepted() {
    decode_payload(&envelope_requiring(None)).expect("an older gateway still syncs");
}

#[test]
fn the_floor_comparison_orders_numerically_not_lexically() {
    assert!(
        bridge_version_is_supported("0.1.10", &semver::Version::new(0, 1, 9)),
        "0.1.10 is newer than 0.1.9; a lexical compare would invert this"
    );
    assert!(!bridge_version_is_supported(
        "0.1.9",
        &semver::Version::new(0, 1, 10)
    ));
    assert!(
        bridge_version_is_supported("1.0.0", &semver::Version::new(1, 0, 0)),
        "the floor itself is supported"
    );
}

#[test]
fn an_unparseable_version_is_refused() {
    assert!(
        !bridge_version_is_supported("dev-build", &semver::Version::new(1, 0, 0)),
        "a version that cannot be parsed cannot be shown to meet the floor; every cargo build \
         carries a semver CARGO_PKG_VERSION, so a work tree is never refused by this"
    );
}

fn marketplace_with_source_ref(
    reference: Option<&str>,
) -> systemprompt_models::bridge::manifest::ManifestMarketplace {
    use systemprompt_models::bridge::manifest::{
        ManifestExternalMarketplace, ManifestExternalMarketplaceSource, ManifestMarketplace,
    };
    ManifestMarketplace {
        id: systemprompt_identifiers::MarketplaceId::new("acme"),
        name: "Acme".to_owned(),
        plugin_ids: Vec::new(),
        allow_cross_marketplace_dependencies_on: Vec::new(),
        external_marketplaces: vec![ManifestExternalMarketplace {
            name: "upstream".to_owned(),
            source: ManifestExternalMarketplaceSource {
                source: "github".to_owned(),
                repo: Some("acme/upstream".to_owned()),
                url: None,
                reference: reference.map(str::to_owned),
            },
        }],
        external_plugins: Vec::new(),
        claude_code: None,
    }
}

#[test]
fn an_external_marketplace_ref_raises_the_manifest_floor() {
    use systemprompt_models::bridge::manifest::{
        EXTERNAL_MARKETPLACE_REF_MIN_BRIDGE, manifest_min_bridge_version,
    };
    let floor = manifest_min_bridge_version(&[
        marketplace_with_source_ref(None),
        marketplace_with_source_ref(Some("b2c-agent-plugins@1.10.0")),
    ]);

    assert_eq!(floor, EXTERNAL_MARKETPLACE_REF_MIN_BRIDGE);
    assert_eq!(floor, semver::Version::new(0, 63, 0));
}

#[test]
fn a_manifest_without_a_ref_keeps_the_default_floor() {
    use systemprompt_models::bridge::manifest::{manifest_min_bridge_version, min_bridge_version};

    assert_eq!(
        manifest_min_bridge_version(&[marketplace_with_source_ref(None)]),
        min_bridge_version()
    );
    assert_eq!(manifest_min_bridge_version(&[]), min_bridge_version());
}

#[test]
fn the_raised_floor_refuses_a_062_bridge_and_admits_063() {
    use systemprompt_models::bridge::manifest::EXTERNAL_MARKETPLACE_REF_MIN_BRIDGE;

    assert!(
        !bridge_version_is_supported("0.62.0", &EXTERNAL_MARKETPLACE_REF_MIN_BRIDGE),
        "a 0.62 bridge parses external sources strictly and must be told to upgrade"
    );
    assert!(bridge_version_is_supported(
        "0.63.0",
        &EXTERNAL_MARKETPLACE_REF_MIN_BRIDGE
    ));
}
