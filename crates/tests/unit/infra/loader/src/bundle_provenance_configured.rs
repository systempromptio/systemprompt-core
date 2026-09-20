use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use systemprompt_loader::bundle::{BundleCache, owning_bundle_hashes, sources_provenance};
use systemprompt_models::profile::FetchFailurePolicy;
use systemprompt_models::services::bundle::{
    BUNDLE_MANIFEST_FILE, BundleSourceState, ServicesBundleState,
};

use crate::bundle_profile::{https_source, profile};
use crate::bundle_support::{pack, pubkey, write};

const CHILD_ENV: &str = "SYSTEMPROMPT_PROVENANCE_CONFIGURED_CHILD";

#[test]
fn configured_bundle_provenance_survives_missing_and_corrupt_manifests_then_recovers() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_child_assertions();
        return;
    }

    let root = tempfile::tempdir().expect("isolated provenance fixture");
    let services = root.path().join("services");
    let cache_root = root.path().join("cache");
    let bundle_tree = root.path().join("bundle-tree");
    let archive = root.path().join("bundle.tar.gz");
    write(&services, "config/config.yaml", "version: 1\n");
    write(
        &bundle_tree,
        "skills/owned-skill/config.yaml",
        "id: owned-skill\nname: Owned skill\n",
    );
    let packed = pack(&bundle_tree, &archive, "4.2.0", ">=0.1");
    let hash = packed.signed.manifest.content_hash.clone();
    let digest = format!("sha256:{}", "a".repeat(64));
    let cache = BundleCache::new(&cache_root);
    let bundle_dir = cache.bundle_dir("kit", &hash);
    std::fs::create_dir_all(&bundle_dir).expect("bundle cache directory");
    let manifest_path = bundle_dir.join(BUNDLE_MANIFEST_FILE);
    let manifest_bytes = serde_json::to_vec(&packed.signed).expect("serialize signed manifest");
    std::fs::write(&manifest_path, &manifest_bytes).expect("cached manifest");
    let manifest_backup = root.path().join("bundle-manifest.backup");
    std::fs::write(&manifest_backup, manifest_bytes).expect("manifest recovery fixture");
    cache
        .write_state(&ServicesBundleState {
            composed_hash: "composed-fixture-hash".to_owned(),
            last_reconciled_hash: None,
            sources: BTreeMap::from([(
                "kit".to_owned(),
                BundleSourceState {
                    digest: digest.clone(),
                    version: "4.2.0".to_owned(),
                    content_hash: hash.clone(),
                    fetched_at: Utc::now(),
                },
            )]),
        })
        .expect("cache state");

    let mut configured = profile(
        &services,
        &cache_root,
        vec![https_source(
            "kit",
            &format!("https://bundles.example/kit@{digest}"),
            vec![pubkey()],
        )],
        FetchFailurePolicy::FailClosed,
    );
    let storage = root.path().join("storage");
    std::fs::create_dir(&storage).expect("profile storage directory");
    configured.paths.storage = Some(storage.to_string_lossy().into_owned());
    let profile_path = root.path().join("profile.yaml");
    std::fs::write(
        &profile_path,
        serde_yaml::to_string(&configured).expect("serialize profile"),
    )
    .expect("profile fixture");

    let stdout = tempfile::NamedTempFile::new().expect("provenance child stdout");
    let stderr = tempfile::NamedTempFile::new().expect("provenance child stderr");
    let mut child = Command::new(std::env::current_exe().expect("current test binary"));
    child
        .args([
            "--exact",
            "bundle_provenance_configured::configured_bundle_provenance_survives_missing_and_corrupt_manifests_then_recovers",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .env("SYSTEMPROMPT_PROFILE", &profile_path)
        .env("PROVENANCE_MANIFEST_PATH", &manifest_path)
        .env("PROVENANCE_MANIFEST_BACKUP", &manifest_backup)
        .stdout(Stdio::from(stdout.reopen().expect("open provenance stdout")))
        .stderr(Stdio::from(stderr.reopen().expect("open provenance stderr")));
    let mut child = child.spawn().expect("run isolated profile child");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll provenance child") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "isolated provenance child timed out\nstdout:\n{}\nstderr:\n{}",
                std::fs::read_to_string(stdout.path()).unwrap_or_default(),
                std::fs::read_to_string(stderr.path()).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(
        status.success(),
        "isolated provenance child failed\nstdout:\n{}\nstderr:\n{}",
        std::fs::read_to_string(stdout.path()).unwrap_or_default(),
        std::fs::read_to_string(stderr.path()).unwrap_or_default()
    );
}

fn run_child_assertions() {
    systemprompt_config::ProfileBootstrap::init().expect("initialize isolated configured profile");
    let provenance = sources_provenance();
    assert_eq!(
        provenance.composed_hash.as_deref(),
        Some("composed-fixture-hash")
    );
    assert_eq!(provenance.bundles.len(), 1);
    let bundle = &provenance.bundles[0];
    assert_eq!(bundle.name, "kit");
    let pinned = format!("sha256:{}", "a".repeat(64));
    assert_eq!(bundle.pinned_digest.as_deref(), Some(pinned.as_str()));
    assert_eq!(bundle.active_digest, bundle.pinned_digest);
    assert_eq!(bundle.version.as_deref(), Some("4.2.0"));
    let hash = bundle.content_hash.clone().expect("active content hash");
    assert_eq!(owning_bundle_hashes().get("owned-skill"), Some(&hash));

    let manifest_path = std::path::PathBuf::from(
        std::env::var_os("PROVENANCE_MANIFEST_PATH").expect("manifest path"),
    );
    std::fs::remove_file(&manifest_path).expect("remove cached manifest");
    assert!(owning_bundle_hashes().is_empty());
    std::fs::write(&manifest_path, b"{ corrupt").expect("corrupt cached manifest");
    assert!(owning_bundle_hashes().is_empty());
    let bytes = std::fs::read(
        std::env::var_os("PROVENANCE_MANIFEST_BACKUP").expect("manifest backup path"),
    )
    .expect("read manifest recovery fixture");
    std::fs::write(&manifest_path, bytes).expect("restore cached manifest");
    assert_eq!(owning_bundle_hashes().get("owned-skill"), Some(&hash));
}
