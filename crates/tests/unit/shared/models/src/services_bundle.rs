use chrono::{TimeZone, Utc};
use systemprompt_models::services::{
    BUNDLE_ALLOWED_DIRS, BUNDLE_FORMAT_VERSION, BUNDLE_MANIFEST_FILE, BUNDLE_MEDIA_TYPE,
    BundleOwnership, BundleSignature, BundleSourceInfo, BundleSourceState, FileEntry,
    MARKETPLACE_BUNDLE_DIRS, ServicesBundleManifest, ServicesBundleState, SignedBundleManifest,
};

fn entry(path: &str, sha256: &str, size: u64) -> FileEntry {
    FileEntry {
        path: path.to_owned(),
        sha256: sha256.to_owned(),
        size,
    }
}

fn manifest(dirs: &[&str]) -> ServicesBundleManifest {
    let files = vec![entry("services/config/config.yaml", "aa", 10)];
    ServicesBundleManifest {
        format: BUNDLE_FORMAT_VERSION,
        version: "1.4.0".to_owned(),
        created_at: Utc.with_ymd_and_hms(2026, 9, 10, 12, 0, 0).unwrap(),
        requires_core: ">=0.49.0".to_owned(),
        source: BundleSourceInfo::default(),
        content_hash: ServicesBundleManifest::compute_content_hash(&files),
        total_size: 10,
        files,
        owns: BundleOwnership {
            dirs: dirs.iter().map(|d| (*d).to_owned()).collect(),
            ..BundleOwnership::default()
        },
    }
}

#[test]
fn constants_describe_the_v1_bundle() {
    assert_eq!(BUNDLE_MANIFEST_FILE, "bundle.json");
    assert_eq!(BUNDLE_FORMAT_VERSION, 1);
    assert!(BUNDLE_MEDIA_TYPE.ends_with(".v1.tar+gzip"));
}

#[test]
fn marketplace_dirs_are_a_subset_of_allowed_dirs() {
    for dir in MARKETPLACE_BUNDLE_DIRS {
        assert!(
            BUNDLE_ALLOWED_DIRS.contains(dir),
            "{dir} is not an allowed bundle directory"
        );
    }
}

#[test]
fn file_entry_serialises_its_digest_as_checksum() {
    let json = serde_json::to_value(entry("a.yaml", "ff", 3)).expect("serialize");
    assert_eq!(json["checksum"], "ff");
    assert!(json.get("sha256").is_none());

    let back: FileEntry = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.sha256, "ff");
}

#[test]
fn file_entry_rejects_unknown_fields() {
    let raw = r#"{"path":"a","checksum":"ff","size":1,"mode":"0644"}"#;
    assert!(serde_json::from_str::<FileEntry>(raw).is_err());
}

#[test]
fn content_hash_is_independent_of_file_order() {
    let a = vec![entry("b.yaml", "22", 2), entry("a.yaml", "11", 1)];
    let b = vec![entry("a.yaml", "11", 1), entry("b.yaml", "22", 2)];
    assert_eq!(
        ServicesBundleManifest::compute_content_hash(&a),
        ServicesBundleManifest::compute_content_hash(&b)
    );
}

#[test]
fn content_hash_changes_when_a_file_digest_changes() {
    let a = vec![entry("a.yaml", "11", 1)];
    let b = vec![entry("a.yaml", "12", 1)];
    assert_ne!(
        ServicesBundleManifest::compute_content_hash(&a),
        ServicesBundleManifest::compute_content_hash(&b)
    );
}

#[test]
fn content_hash_distinguishes_a_rename_from_a_digest_swap() {
    let a = vec![entry("a", "11", 1), entry("b", "22", 1)];
    let b = vec![entry("a", "22", 1), entry("b", "11", 1)];
    assert_ne!(
        ServicesBundleManifest::compute_content_hash(&a),
        ServicesBundleManifest::compute_content_hash(&b)
    );
}

#[test]
fn content_hash_of_no_files_is_stable() {
    assert_eq!(
        ServicesBundleManifest::compute_content_hash(&[]),
        ServicesBundleManifest::compute_content_hash(&[])
    );
}

#[test]
fn marketplace_only_bundle_owns_only_marketplace_dirs() {
    assert!(manifest(&["marketplaces", "plugins", "skills"]).is_marketplace_only());
}

#[test]
fn a_bundle_claiming_a_base_dir_is_not_marketplace_only() {
    assert!(!manifest(&["marketplaces", "mcp"]).is_marketplace_only());
    assert!(!manifest(&["gateway"]).is_marketplace_only());
}

#[test]
fn a_bundle_owning_no_dirs_is_not_marketplace_only() {
    assert!(!manifest(&[]).is_marketplace_only());
}

#[test]
fn core_satisfies_accepts_a_matching_version() {
    let m = manifest(&["marketplaces"]);
    assert!(m.core_satisfies("0.49.0").expect("semver"));
    assert!(m.core_satisfies("0.50.1").expect("semver"));
    assert!(!m.core_satisfies("0.48.9").expect("semver"));
}

#[test]
fn core_satisfies_reports_an_unparseable_requirement() {
    let mut m = manifest(&["marketplaces"]);
    m.requires_core = "not-a-req".to_owned();
    assert!(m.core_satisfies("0.49.0").is_err());
}

#[test]
fn core_satisfies_reports_an_unparseable_core_version() {
    assert!(
        manifest(&["marketplaces"])
            .core_satisfies("nightly")
            .is_err()
    );
}

#[test]
fn signed_manifest_round_trips_with_and_without_a_signature() {
    let signed = SignedBundleManifest {
        manifest: manifest(&["marketplaces"]),
        signature: Some(BundleSignature {
            alg: "ed25519".to_owned(),
            key_id: "0123456789abcdef".to_owned(),
            sig_b64: "c2ln".to_owned(),
        }),
    };
    let json = serde_json::to_string(&signed).expect("serialize");
    let back: SignedBundleManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, signed);

    let unsigned = r#"{"manifest":"#.to_owned()
        + &serde_json::to_string(&signed.manifest).expect("serialize")
        + "}";
    let back: SignedBundleManifest = serde_json::from_str(&unsigned).expect("deserialize");
    assert!(back.signature.is_none());
}

#[test]
fn manifest_rejects_an_unknown_field() {
    let mut json = serde_json::to_value(manifest(&["marketplaces"])).expect("serialize");
    json["signed_by"] = serde_json::Value::String("someone".to_owned());
    assert!(serde_json::from_value::<ServicesBundleManifest>(json).is_err());
}

#[test]
fn bundle_state_round_trips_per_source_entries() {
    let mut state = ServicesBundleState {
        composed_hash: "abc".to_owned(),
        last_reconciled_hash: Some("abc".to_owned()),
        ..ServicesBundleState::default()
    };
    state.sources.insert(
        "base".to_owned(),
        BundleSourceState {
            digest: "sha256:deadbeef".to_owned(),
            version: "1.4.0".to_owned(),
            content_hash: "abc".to_owned(),
            fetched_at: Utc.with_ymd_and_hms(2026, 9, 10, 12, 0, 0).unwrap(),
        },
    );

    let json = serde_json::to_string(&state).expect("serialize");
    let back: ServicesBundleState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, state);
}

#[test]
fn bundle_state_defaults_when_no_source_has_been_fetched() {
    let back: ServicesBundleState = serde_json::from_str("{}").expect("deserialize");
    assert!(back.sources.is_empty());
    assert!(back.last_reconciled_hash.is_none());
    assert!(back.composed_hash.is_empty());
}
