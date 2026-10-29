use std::path::{Path, PathBuf};

use systemprompt_bridge::config::gateways::credential::{self, PatStore};
use systemprompt_bridge::config::gateways::migrate::{
    MigrationOutcome, migrate_legacy_layout, migrate_legacy_layout_with,
};
use systemprompt_bridge::config::gateways::{self, GatewayProfileId, ProfileError};
use tempfile::TempDir;

const LEGACY_PAT: &str = "sp-live-legacy0.abcdefghijklmnopqrstuvwxyz0123456789";
const TRUST_KEY: &str = "WGZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY=";

fn sandbox(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/config"))),
        ("XDG_STATE_HOME", Some(format!("{root}/state"))),
        ("XDG_DATA_HOME", Some(format!("{root}/data"))),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
    ]
}

fn root(home: &TempDir) -> PathBuf {
    home.path().join("config").join("systemprompt")
}

fn index(home: &TempDir) -> PathBuf {
    root(home).join("systemprompt-bridge.toml")
}

fn default_profile(home: &TempDir) -> PathBuf {
    root(home).join("gateways").join("default")
}

fn seed_legacy(home: &TempDir) -> PathBuf {
    let dir = root(home);
    std::fs::create_dir_all(&dir).unwrap();
    let pat = dir.join("systemprompt-bridge.pat");
    std::fs::write(&pat, LEGACY_PAT).unwrap();
    std::fs::write(
        index(home),
        format!(
            "# operator note\ngateway_url = \"https://gw.example.com\"\n[pat]\nfile = '{}'\n\
             [sync.trust]\ngateway = 'https://gw.example.com'\nkey = '{TRUST_KEY}'\nsource = 'operator'\n",
            pat.display()
        ),
    )
    .unwrap();
    pat
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_fresh_install_has_nothing_to_migrate_and_resolves_the_default_profile() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        assert_eq!(migrate_legacy_layout().unwrap(), MigrationOutcome::Fresh);
        let active = gateways::active().unwrap();
        assert_eq!(active.id, GatewayProfileId::default_profile());
        assert_eq!(
            active.config_file,
            default_profile(&home).join("systemprompt-bridge.toml")
        );
        assert!(!index(&home).exists(), "a fresh install writes no index");
    });
}

#[test]
fn the_legacy_layout_moves_into_the_default_profile_with_its_trust_and_pat() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        let legacy_pat = seed_legacy(&home);

        let outcome = migrate_legacy_layout_with(PatStore::File).unwrap();
        assert_eq!(
            outcome,
            MigrationOutcome::Migrated {
                profile: GatewayProfileId::default_profile(),
                pat: Some(PatStore::File),
            }
        );

        let index_body = read(&index(&home));
        assert!(index_body.contains("active = \"default\""), "{index_body}");
        assert!(!index_body.contains("gateway_url"), "{index_body}");

        let profile_cfg = read(&default_profile(&home).join("systemprompt-bridge.toml"));
        assert!(
            profile_cfg.contains("# operator note"),
            "comments survive: {profile_cfg}"
        );
        assert!(profile_cfg.contains("https://gw.example.com"));
        assert!(
            profile_cfg.contains(TRUST_KEY),
            "the trust record moves with the config"
        );
        assert!(profile_cfg.contains("store = \"file\""), "{profile_cfg}");
        assert!(
            !profile_cfg.contains("file = '"),
            "the legacy pat path is dropped"
        );

        assert!(
            !legacy_pat.exists(),
            "the legacy PAT file is removed once stored"
        );
        let profile = gateways::active().unwrap();
        assert_eq!(
            credential::read(&profile, PatStore::File)
                .unwrap()
                .unwrap()
                .as_str(),
            LEGACY_PAT
        );
        let cfg = systemprompt_bridge::config::load().unwrap();
        assert_eq!(cfg.pat.and_then(|p| p.store), Some(PatStore::File));
        assert_eq!(
            cfg.gateway_url.unwrap().as_str().trim_end_matches('/'),
            "https://gw.example.com"
        );

        assert_eq!(
            migrate_legacy_layout_with(PatStore::File).unwrap(),
            MigrationOutcome::AlreadyMigrated,
            "a second run is a no-op"
        );
    });
}

#[test]
fn a_move_interrupted_before_the_index_is_written_completes_on_the_next_run() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        let legacy_pat = seed_legacy(&home);
        let partial = default_profile(&home);
        std::fs::create_dir_all(&partial).unwrap();
        std::fs::write(partial.join("systemprompt-bridge.toml"), "half = written").unwrap();

        let outcome = migrate_legacy_layout_with(PatStore::File).unwrap();
        assert!(
            matches!(outcome, MigrationOutcome::Migrated { .. }),
            "{outcome:?}"
        );
        assert!(read(&partial.join("systemprompt-bridge.toml")).contains("gw.example.com"));
        assert!(!legacy_pat.exists());
    });
}

#[test]
fn a_legacy_pat_left_after_the_commit_is_removed_only_when_it_matches_the_stored_one() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        let legacy_pat = seed_legacy(&home);
        migrate_legacy_layout_with(PatStore::File).unwrap();

        std::fs::write(&legacy_pat, LEGACY_PAT).unwrap();
        assert_eq!(
            migrate_legacy_layout_with(PatStore::File).unwrap(),
            MigrationOutcome::AlreadyMigrated
        );
        assert!(!legacy_pat.exists(), "the matching leftover is cleaned up");

        std::fs::write(
            &legacy_pat,
            "sp-live-someone-else.0000000000000000000000000000",
        )
        .unwrap();
        migrate_legacy_layout_with(PatStore::File).unwrap();
        assert!(
            legacy_pat.exists(),
            "a PAT that differs is left alone, not deleted"
        );
    });
}

#[test]
fn a_pinned_config_is_never_migrated_and_refuses_profile_changes() {
    let home = TempDir::new().unwrap();
    let pinned = home.path().join("pinned.toml");
    let mut vars = sandbox(&home);
    vars.retain(|(k, _)| *k != "SP_BRIDGE_CONFIG");
    vars.push((
        "SP_BRIDGE_CONFIG",
        Some(pinned.to_string_lossy().into_owned()),
    ));
    temp_env::with_vars(vars, || {
        seed_legacy(&home);
        assert_eq!(migrate_legacy_layout().unwrap(), MigrationOutcome::Pinned);
        let active = gateways::active().unwrap();
        assert!(active.pinned);
        assert_eq!(active.config_file, pinned);
        assert!(matches!(
            gateways::add("https://other.example.com"),
            Err(ProfileError::Pinned { .. })
        ));
    });
}

#[test]
fn profiles_are_added_listed_switched_and_removed() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        seed_legacy(&home);
        migrate_legacy_layout_with(PatStore::File).unwrap();

        let id = gateways::add("https://client-b.example.com").unwrap();
        assert_eq!(id.as_str(), "client-b-example-com");
        assert!(matches!(
            gateways::add("https://client-b.example.com/"),
            Err(ProfileError::Duplicate(..))
        ));

        let listed = gateways::list().unwrap();
        let ids: Vec<&str> = listed.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["client-b-example-com", "default"]);
        assert!(
            listed
                .iter()
                .any(|p| p.active && p.id.as_str() == "default")
        );

        gateways::set_active(&id).unwrap();
        assert_eq!(gateways::active().unwrap().id, id);
        assert_eq!(
            systemprompt_bridge::config::load()
                .unwrap()
                .gateway_url
                .unwrap()
                .as_str()
                .trim_end_matches('/'),
            "https://client-b.example.com"
        );
        assert!(matches!(
            gateways::remove(&id),
            Err(ProfileError::RemoveActive(_))
        ));

        let default = GatewayProfileId::default_profile();
        gateways::set_active(&default).unwrap();
        gateways::remove(&id).unwrap();
        assert!(!root(&home).join("gateways").join(id.as_str()).exists());
        assert!(matches!(
            gateways::set_active(&id),
            Err(ProfileError::Unknown(_))
        ));
    });
}

#[test]
fn profile_ids_refuse_path_and_account_metacharacters() {
    for bad in ["", "../x", "A", "a/b", "-lead", "a b"] {
        assert!(GatewayProfileId::try_new(bad).is_err(), "{bad:?}");
    }
    assert!(GatewayProfileId::try_new("client-b-2").is_ok());
}

#[test]
fn the_file_store_writes_a_private_file_inside_the_profile() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox(&home), || {
        let profile = gateways::active().unwrap();
        std::fs::create_dir_all(&profile.dir).unwrap();
        credential::write(&profile, PatStore::File, "  sp-live-x.y  ").unwrap();
        let path = profile.pat_file();
        assert!(path.starts_with(&profile.dir));
        assert_eq!(read(&path), "sp-live-x.y");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        credential::delete(&profile, PatStore::File).unwrap();
        assert!(
            matches!(
                credential::read(&profile, PatStore::File),
                Err(credential::CredentialError::File { action: "read", .. })
            ),
            "a recorded file store with no file is a missing credential, not an absent one"
        );
    });
}

// Why: Linux without a reachable Secret Service (headless, WSL, CI) must keep
// the PAT in the profile file, never in the kernel keyutils store that forgets
// it on reboot.
#[cfg(target_os = "linux")]
#[test]
fn linux_without_a_session_bus_selects_the_file_store() {
    temp_env::with_vars(
        [
            ("DBUS_SESSION_BUS_ADDRESS", None::<&str>),
            ("DISPLAY", None),
            ("XDG_RUNTIME_DIR", Some("/nonexistent-runtime-dir")),
        ],
        || assert_eq!(credential::select_store(), PatStore::File),
    );
}
