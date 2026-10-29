use systemprompt_manifest::services::gateway::GatewayProfileError;
use systemprompt_manifest::services::{GatewayConfig, GatewayConfigSpec};

fn gateway(yaml: &str) -> GatewayConfig {
    serde_yaml::from_str::<GatewayConfigSpec>(yaml)
        .expect("gateway spec parses")
        .resolve()
}

fn releases(window: &str) -> GatewayConfig {
    gateway(&format!(
        "bridge_releases:\n  repo: systempromptio/bridge\n{window}"
    ))
}

#[test]
fn a_bridge_instance_with_relative_paths_and_branding_validates() {
    let config = gateway(
        "bridge_instance:\n  device_link_path: /bridge-auth/device-link\n  identity_path: \
         /api/public/bridge/whoami\n  branding:\n    display_name: Acme\n    contact_email: \
         it@acme.test\n",
    );
    config.validate().expect("relative paths validate");
    let instance = config.bridge_instance.expect("block present");
    let branding = instance.branding.to_branding();
    assert_eq!(branding.display_name.as_deref(), Some("Acme"));
    assert_eq!(branding.contact_email.as_deref(), Some("it@acme.test"));
    assert!(branding.logo_url.is_none());
}

#[test]
fn a_bridge_instance_path_with_a_host_is_refused() {
    for path in ["https://evil.test/link", "//evil.test/link", "bridge/link"] {
        let config = gateway(&format!(
            "bridge_instance:\n  device_link_path: '{path}'\n  identity_path: /whoami\n"
        ));
        let error = config.validate().expect_err(path);
        assert!(
            matches!(
                error,
                GatewayProfileError::BridgeInstancePathNotRelative {
                    field: "device_link_path",
                    ref value,
                } if value == path
            ),
            "{error:?}"
        );
    }
}

#[test]
fn a_bridge_instance_rejects_unknown_keys() {
    let parsed = serde_yaml::from_str::<GatewayConfigSpec>(
        "bridge_instance:\n  device_link_path: /a\n  identity_path: /b\n  colour: red\n",
    );
    assert!(parsed.is_err());
}

#[test]
fn a_bridge_instance_without_an_identity_path_does_not_parse() {
    let parsed =
        serde_yaml::from_str::<GatewayConfigSpec>("bridge_instance:\n  device_link_path: /a\n");
    assert!(parsed.is_err());
}

#[test]
fn a_release_window_with_a_pin_inside_it_validates() {
    let config = releases(
        "  min_version: 0.74.0\n  max_version: 0.75.3\n  enforce_max: true\n  pinned_version: \
         0.75.1\n",
    );
    config.validate().expect("pin sits inside the window");
    let window = config
        .bridge_releases
        .expect("releases present")
        .version_window();
    assert_eq!(window.min, Some(semver::Version::new(0, 74, 0)));
    assert_eq!(window.max, Some(semver::Version::new(0, 75, 3)));
    assert!(window.enforce_max);
}

#[test]
fn a_window_bound_with_build_metadata_is_refused() {
    let error = releases("  min_version: 0.74.0+dynapps.2\n")
        .validate()
        .expect_err("build metadata is ignored by precedence");
    assert!(
        matches!(
            error,
            GatewayProfileError::BridgeVersionBuildMetadata {
                field: "min_version",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn an_inverted_window_is_refused() {
    let error = releases("  min_version: 0.76.0\n  max_version: 0.75.0\n")
        .validate()
        .expect_err("min above max");
    assert!(
        matches!(
            error,
            GatewayProfileError::BridgeVersionWindowInverted { .. }
        ),
        "{error:?}"
    );
}

#[test]
fn a_pin_outside_the_window_is_refused() {
    let error = releases("  max_version: 0.75.0\n  pinned_version: 0.75.1\n")
        .validate()
        .expect_err("pin above max");
    assert!(
        matches!(
            error,
            GatewayProfileError::BridgePinnedVersionOutsideWindow { ref pinned }
                if *pinned == semver::Version::new(0, 75, 1)
        ),
        "{error:?}"
    );
}

#[test]
fn a_pin_that_is_not_semver_is_refused() {
    let error = releases("  pinned_version: latest\n")
        .validate()
        .expect_err("not semver");
    assert!(
        matches!(error, GatewayProfileError::BridgePinnedVersionInvalid { ref value, .. } if value == "latest"),
        "{error:?}"
    );
}
