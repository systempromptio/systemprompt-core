use std::path::PathBuf;

use systemprompt_bridge::gateway::manifest::{ArtifactEntry, SignedManifest};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{LibraryArtifactId, ManifestSignature, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::CoworkArtifactsSync;
use systemprompt_bridge::integration::cowork_artifacts::emit::{
    active_sinks, resolve_artifacts_dir, write_artifacts,
};
use systemprompt_bridge::integration::cowork_artifacts::sink::LIBRARY_STORE_FILE;
use systemprompt_bridge::sync::{HostSync, HostSyncCtx};
use systemprompt_test_fixtures::fixture_user_id;
use tempfile::TempDir;

fn artifact(id: &str, version: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).unwrap(),
        name: id.to_owned(),
        description: "desc".into(),
        version: version.to_owned(),
        mcp_tools: vec![],
        content: "<p/>".into(),
        starred: false,
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
    }
}

fn manifest(artifacts: Vec<ArtifactEntry>) -> SignedManifest {
    SignedManifest {
        manifest_version: ManifestVersion::try_new("2026-05-01T12:00:00Z-deadbeef").unwrap(),
        issued_at: "2026-05-01T12:00:00+00:00".into(),
        not_before: "2026-05-01T12:00:00+00:00".into(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec!["cowork".into()],
        host_model_protocols: Default::default(),
        artifacts,
        signature: ManifestSignature::new(""),
    }
}

struct Session {
    dir: TempDir,
}

impl Session {
    fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(
            dir.path()
                .join("Claude-3p")
                .join("local-agent-mode-sessions")
                .join("account")
                .join("00000000-0000-4000-8000-000000000001")
                .join("cowork_plugins"),
        )
        .expect("cowork session tree");
        Self { dir }
    }

    fn artifacts_dir(&self) -> PathBuf {
        self.dir
            .path()
            .join("Claude-3p")
            .join("local-agent-mode-sessions")
            .join("account")
            .join("00000000-0000-4000-8000-000000000001")
            .join("cowork_artifacts")
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let root = self.dir.path().display().to_string();
        temp_env::with_vars(
            vec![
                ("XDG_CONFIG_HOME", Some(root.clone())),
                ("HOME", Some(root)),
                ("SP_BRIDGE_CONFIG", None),
            ],
            f,
        )
    }
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

fn stub_ctx<'a>(
    m: &'a SignedManifest,
    root: &'a std::path::Path,
    client: &'a systemprompt_bridge::gateway::GatewayClient,
    servers: &'a std::collections::BTreeMap<String, Vec<String>>,
) -> HostSyncCtx<'a> {
    HostSyncCtx {
        manifest: m,
        org_plugins_root: root,
        plugin_mcp_servers: servers,
        client,
        bearer: "",
    }
}

fn client() -> systemprompt_bridge::gateway::GatewayClient {
    systemprompt_bridge::gateway::GatewayClient::new(
        systemprompt_identifiers::ValidatedUrl::try_new("http://127.0.0.1:0").unwrap(),
    )
}

#[test]
fn the_emitter_resolves_the_artifacts_dir_from_the_cowork_session() {
    let session = Session::new();
    let resolved = session
        .run(resolve_artifacts_dir)
        .expect("session detected");
    assert_eq!(resolved, session.artifacts_dir());
}

#[test]
fn without_a_cowork_install_apply_and_clear_are_no_ops() {
    let empty = TempDir::new().expect("tempdir");
    let root = empty.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("XDG_CONFIG_HOME", Some(root.clone())),
            ("HOME", Some(root)),
            ("SP_BRIDGE_CONFIG", None),
        ],
        || {
            assert!(resolve_artifacts_dir().is_none());
            let m = manifest(vec![artifact("pipeline", "1")]);
            let servers = std::collections::BTreeMap::new();
            let c = client();
            block_on(CoworkArtifactsSync.apply(&stub_ctx(&m, empty.path(), &c, &servers)))
                .expect("apply is a no-op without Cowork");
            CoworkArtifactsSync
                .clear()
                .expect("clear is a no-op without Cowork");
        },
    );
}

#[test]
fn apply_writes_the_store_and_clear_removes_the_whole_directory() {
    let session = Session::new();
    session.run(|| {
        let m = manifest(vec![artifact("pipeline", "1")]);
        let servers = std::collections::BTreeMap::new();
        let c = client();
        block_on(CoworkArtifactsSync.apply(&stub_ctx(&m, session.dir.path(), &c, &servers)))
            .expect("apply writes the store");
        assert!(
            session.artifacts_dir().join(LIBRARY_STORE_FILE).is_file(),
            "the library store is materialised"
        );

        CoworkArtifactsSync
            .clear()
            .expect("clear removes the store");
        assert!(
            !session.artifacts_dir().exists(),
            "an explicit teardown removes the whole artifacts dir"
        );
        CoworkArtifactsSync
            .clear()
            .expect("a second clear is idempotent");
    });
}

#[test]
fn an_empty_artifact_set_preserves_an_existing_store_rather_than_clearing_it() {
    let session = Session::new();
    session.run(|| {
        let dir = session.artifacts_dir();
        write_artifacts(&dir, active_sinks(), &[artifact("pipeline", "1")]).expect("seed store");
        let before = std::fs::read(dir.join(LIBRARY_STORE_FILE)).expect("store");

        let m = manifest(vec![]);
        let servers = std::collections::BTreeMap::new();
        let c = client();
        block_on(CoworkArtifactsSync.apply(&stub_ctx(&m, session.dir.path(), &c, &servers)))
            .expect("an empty set is not an error");

        assert_eq!(
            std::fs::read(dir.join(LIBRARY_STORE_FILE)).expect("store"),
            before,
            "an enabled host sending zero artifacts must never wipe the user's library"
        );
    });
}
