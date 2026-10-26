//! The `OpenCode` client version gate: the bridge writes V2 config only, so a
//! client below the V2 floor is an error in doctor.

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::opencode::check_client_version;
use systemprompt_bridge::integration::opencode::{
    ClientVersion, MIN_CLIENT_VERSION, parse_version,
};

#[test]
fn version_output_shapes_parse() {
    assert_eq!(
        parse_version("2.0.31\n"),
        Some(semver::Version::new(2, 0, 31))
    );
    assert_eq!(
        parse_version("opencode v2.1.0"),
        Some(semver::Version::new(2, 1, 0))
    );
    assert_eq!(parse_version("no version here"), None);
}

#[test]
fn a_client_below_the_v2_floor_fails_doctor() {
    let old = check_client_version(&ClientVersion::Found(semver::Version::new(2, 0, 29)))
        .expect("a found client is reported");
    assert_eq!(old.status, Status::Fail);
    assert!(
        old.detail.contains(&MIN_CLIENT_VERSION.to_string()),
        "{}",
        old.detail
    );
    let current =
        check_client_version(&ClientVersion::Found(MIN_CLIENT_VERSION)).expect("reported");
    assert_eq!(current.status, Status::Ok);
    assert!(check_client_version(&ClientVersion::NotInstalled).is_none());
    let unreadable =
        check_client_version(&ClientVersion::Unreadable("timed out".into())).expect("reported");
    assert_eq!(unreadable.status, Status::Warn);
}

#[test]
fn the_version_is_probed_only_for_an_enabled_opencode_packaging() {
    use systemprompt_bridge::cli::doctor::opencode::{checks, opencode_enabled};
    use systemprompt_models::bridge::host::HostKind;
    assert!(opencode_enabled(&[HostKind::OpenCodeCli]));
    assert!(opencode_enabled(&[
        HostKind::ClaudeCode,
        HostKind::OpenCode
    ]));
    assert!(!opencode_enabled(&[
        HostKind::ClaudeCode,
        HostKind::CodexCli
    ]));
    let unprobed = |enabled: Option<&[HostKind]>| {
        checks(enabled)
            .iter()
            .all(|check| check.name != "opencode version")
    };
    assert!(unprobed(None));
    assert!(unprobed(Some(&[HostKind::ClaudeCode])));
}
