//! A generated host profile carries a credential derived from the loopback
//! secret: it is written owner-only under the brand's temp dir, its UUIDs are
//! real UUIDs, and installing it consumes the file.

use std::path::Path;

use systemprompt_bridge::ids::LoopbackSecret;
use systemprompt_bridge::integration::generated_profile::{self, profile_uuids};
use systemprompt_bridge::integration::hermes::HERMES_HOST;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs};

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        api_key: LoopbackSecret::new("loopback-secret-value"),
        models: vec!["gpt-5".to_owned()],
        default_model: None,
        organization_uuid: None,
        headers: Default::default(),
        mcp_servers: Some(Vec::new()),
    }
}

#[test]
fn profile_uuids_are_uppercase_v4_uuids_and_differ_from_each_other() {
    let uuids = profile_uuids();
    for raw in [&uuids.payload, &uuids.profile] {
        let parsed = uuid::Uuid::parse_str(raw).expect("a parseable UUID");
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random), "{raw}");
        assert_eq!(
            *raw,
            raw.to_ascii_uppercase(),
            "mobileconfig UUIDs are uppercase"
        );
    }
    assert_ne!(uuids.payload, uuids.profile);
    assert_ne!(profile_uuids(), uuids, "every call mints fresh identifiers");
}

#[cfg(unix)]
#[test]
fn a_written_profile_is_owner_only_and_consumed_after_install() {
    use std::os::unix::fs::PermissionsExt;

    let path = generated_profile::write("privacy-probe", ".yaml", b"body: 1\n").expect("written");
    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "{}: {mode:o}", path.display());
    let dir_mode = std::fs::metadata(path.parent().expect("parent"))
        .expect("dir metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700, "the brand temp dir is private");

    generated_profile::consume(&path.display().to_string()).expect("consumed");
    assert!(!path.exists());
    generated_profile::consume(&path.display().to_string())
        .expect("consuming an already-consumed profile is idempotent");
}

#[cfg(unix)]
#[test]
fn a_host_generated_profile_is_private_and_gone_once_installed() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let home = temp.path().display().to_string();
    temp_env::with_var("HERMES_HOME", Some(home.as_str()), || {
        let generated = HERMES_HOST
            .generate_profile(&inputs())
            .expect("profile generated");
        let mode = std::fs::metadata(&generated.path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{}", generated.path);

        HERMES_HOST
            .install_profile(&generated.path)
            .expect("install merges into config.yaml");
        assert!(
            !Path::new(&generated.path).exists(),
            "the installer consumes the generated profile"
        );
    });
}
