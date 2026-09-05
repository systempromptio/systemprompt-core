use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, SignedManifest, SkillEntry, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::host_sync::{ApplyError, HostSync, HostSyncCtx};
use systemprompt_bridge::ids::{Sha256Digest, SkillId, SkillName};
use systemprompt_bridge::integration::hermes::HermesSync;
use systemprompt_bridge::integration::opencode::OpenCodeSync;
use systemprompt_bridge::proxy::LoopbackEndpoint;

static EMPTY_REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);

static LOOPBACK: std::sync::LazyLock<LoopbackEndpoint> = std::sync::LazyLock::new(|| {
    LoopbackEndpoint::new(systemprompt_bridge::proxy::DEFAULT_PROXY_PORT, None)
});

struct Sandbox {
    hermes_skills: PathBuf,
    opencode_skills: PathBuf,
    org_plugins: PathBuf,
}

fn with_sandbox<R>(body: impl FnOnce(&Sandbox) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let base = temp.path();
    let hermes_home = base.join("hermes");
    let config_home = base.join("config");
    let org_plugins = base.join("org-plugins");
    for d in [&hermes_home, &config_home, &org_plugins] {
        fs::create_dir_all(d).expect("sandbox dir");
    }
    let sb = Sandbox {
        hermes_skills: hermes_home.join("skills"),
        opencode_skills: config_home.join("opencode").join("skills"),
        org_plugins,
    };
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HERMES_HOME", Some(hermes_home.display().to_string())),
        ("HOME", Some(base.display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        (
            "XDG_DATA_HOME",
            Some(base.join("data").display().to_string()),
        ),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(base.join("managed").display().to_string()),
        ),
        ("SP_BRIDGE_CONFIG", None),
    ];
    let out = temp_env::with_vars(vars, || body(&sb));
    drop(temp);
    out
}

fn skill(id: &str, hosts: &[&str], instructions: &str) -> SkillEntry {
    SkillEntry {
        id: SkillId::try_new(id).expect("skill id"),
        name: SkillName::try_new(id).expect("skill name"),
        description: format!("desc for {id}"),
        file_path: format!("{id}/SKILL.md"),
        tags: vec![],
        sha256: Sha256Digest::try_new("0".repeat(64)).expect("digest"),
        instructions: instructions.to_owned(),
        hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        plugins: vec![],
    }
}

fn manifest(skills: Vec<SkillEntry>) -> SignedManifest {
    SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: None,
        manifest_version: ManifestVersion::try_new("2026-04-30T12:00:00Z-deadbeef")
            .expect("manifest version"),
        issued_at: "2026-04-30T12:00:00+00:00".into(),
        not_before: "2026-04-30T12:00:00+00:00".into(),
        user_id: systemprompt_identifiers::UserId::new("test-user"),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills,
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec!["hermes".into(), "opencode".into()],
        host_model_protocols: BTreeMap::new(),
        artifacts: vec![],
        allow_claude_ai_connectors: false,
        diagnostics: Vec::new(),
        marketplaces: Vec::new(),
    }
}

fn stub_client() -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new("http://127.0.0.1:0").expect("stub url"),
        reqwest::Client::new(),
    )
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(f)
}

fn apply<H: HostSync>(host: &H, m: &SignedManifest, sb: &Sandbox) -> Result<(), ApplyError> {
    let plugin_mcp_servers = BTreeMap::new();
    let client = stub_client();
    let ctx = HostSyncCtx {
        manifest: m,
        org_plugins_root: sb.org_plugins.as_path(),
        plugin_mcp_servers: &plugin_mcp_servers,
        client: &client,
        bearer: "",
        loopback: &LOOPBACK,
        mcp_registry: &EMPTY_REGISTRY,
    };
    block_on(host.apply(&ctx))
}

fn dirs_in(root: &Path) -> Vec<String> {
    if !root.exists() {
        return Vec::new();
    }
    let mut out: Vec<String> = fs::read_dir(root)
        .expect("read skills dir")
        .map(|e| e.expect("dir entry"))
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

fn sidecar_ids(root: &Path) -> Vec<String> {
    let raw =
        fs::read_to_string(root.join(".systemprompt-managed.json")).expect("sidecar readable");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("sidecar is json");
    value["ids"]
        .as_array()
        .unwrap_or_else(|| panic!("sidecar has an ids array: {raw}"))
        .iter()
        .map(|v| v.as_str().expect("id is a string").to_owned())
        .collect()
}

#[test]
fn a_skill_with_no_host_list_lands_on_every_host() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill("shared", &[], "# Shared\n")]);
        apply(&HermesSync, &m, sb).expect("hermes apply");
        apply(&OpenCodeSync, &m, sb).expect("opencode apply");
        assert_eq!(dirs_in(&sb.hermes_skills), vec!["shared".to_owned()]);
        assert_eq!(dirs_in(&sb.opencode_skills), vec!["shared".to_owned()]);
    });
}

#[test]
fn a_skill_aimed_at_another_host_never_lands() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill("opencode-only", &["opencode"], "# OC\n")]);
        apply(&HermesSync, &m, sb).expect("hermes apply");
        apply(&OpenCodeSync, &m, sb).expect("opencode apply");
        assert!(
            dirs_in(&sb.hermes_skills).is_empty(),
            "hermes was not targeted, got {:?}",
            dirs_in(&sb.hermes_skills)
        );
        assert_eq!(
            dirs_in(&sb.opencode_skills),
            vec!["opencode-only".to_owned()],
            "the targeted host still receives it"
        );
    });
}

#[test]
fn a_host_targeted_by_name_receives_the_skill() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill(
            "hermes-only",
            &["hermes", "codex-cli"],
            "# H\n",
        )]);
        apply(&HermesSync, &m, sb).expect("hermes apply");
        assert_eq!(dirs_in(&sb.hermes_skills), vec!["hermes-only".to_owned()]);
        assert_eq!(
            sidecar_ids(&sb.hermes_skills),
            vec!["hermes-only".to_owned()]
        );
    });
}

#[test]
fn a_skill_that_stops_targeting_a_host_is_pruned_from_it() {
    with_sandbox(|sb| {
        let first = manifest(vec![
            skill("keeper", &[], "# Keep\n"),
            skill("dropped", &["hermes"], "# Drop\n"),
        ]);
        apply(&HermesSync, &first, sb).expect("first apply");
        assert_eq!(
            dirs_in(&sb.hermes_skills),
            vec!["dropped".to_owned(), "keeper".to_owned()]
        );

        let second = manifest(vec![
            skill("keeper", &[], "# Keep\n"),
            skill("dropped", &["opencode"], "# Drop\n"),
        ]);
        apply(&HermesSync, &second, sb).expect("second apply");
        assert_eq!(
            dirs_in(&sb.hermes_skills),
            vec!["keeper".to_owned()],
            "a skill re-aimed at another host is removed from this one"
        );
        assert_eq!(sidecar_ids(&sb.hermes_skills), vec!["keeper".to_owned()]);
    });
}

#[test]
fn a_user_authored_skill_the_sidecar_never_claimed_survives_a_prune() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill("managed", &[], "# Managed\n")]);
        apply(&HermesSync, &m, sb).expect("first apply");
        let mine = sb.hermes_skills.join("mine");
        fs::create_dir_all(&mine).expect("user skill dir");
        fs::write(mine.join("SKILL.md"), "# Mine\n").expect("user skill");

        apply(&HermesSync, &manifest(vec![]), sb).expect("empty apply");
        assert_eq!(
            dirs_in(&sb.hermes_skills),
            vec!["mine".to_owned()],
            "only the managed dir was pruned"
        );
        assert_eq!(
            fs::read_to_string(mine.join("SKILL.md")).expect("user skill intact"),
            "# Mine\n"
        );
    });
}

#[test]
fn a_kebab_host_renames_the_directory_and_the_front_matter_name_with_it() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill(
            "Deep_Research",
            &["opencode"],
            "---\nname: upstream-name\ndescription: from upstream\n---\n\n# Body\n",
        )]);
        apply(&OpenCodeSync, &m, sb).expect("opencode apply");
        assert_eq!(
            dirs_in(&sb.opencode_skills),
            vec!["deep-research".to_owned()]
        );
        let body = fs::read_to_string(sb.opencode_skills.join("deep-research").join("SKILL.md"))
            .expect("SKILL.md");
        assert!(
            body.contains("name: deep-research"),
            "the front matter name must match the folder: {body}"
        );
        assert!(
            !body.contains("upstream-name"),
            "the upstream name is replaced, not trusted: {body}"
        );
        assert!(body.ends_with("# Body\n"), "{body}");
    });
}

#[test]
fn a_verbatim_host_keeps_the_upstream_front_matter_untouched() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill(
            "research",
            &["hermes"],
            "---\nname: upstream-name\n---\n\n# Body\n",
        )]);
        apply(&HermesSync, &m, sb).expect("hermes apply");
        let body = fs::read_to_string(sb.hermes_skills.join("research").join("SKILL.md"))
            .expect("SKILL.md");
        assert!(
            body.contains("name: upstream-name"),
            "hermes reads the folder verbatim, so the author's front matter stands: {body}"
        );
    });
}

#[test]
fn two_skills_that_collapse_onto_one_directory_are_refused() {
    with_sandbox(|sb| {
        let m = manifest(vec![
            skill("Deep_Research", &["opencode"], "# A\n"),
            skill("deep-research", &["opencode"], "# B\n"),
        ]);
        let err = apply(&OpenCodeSync, &m, sb).expect_err("a collision must not be written");
        match err {
            ApplyError::SkillDirCollision { dir, .. } => assert_eq!(dir, "deep-research"),
            other => panic!("expected SkillDirCollision, got {other:?}"),
        }
        assert!(
            dirs_in(&sb.opencode_skills).is_empty(),
            "nothing is written when the selection is refused"
        );
    });
}

#[test]
fn an_identical_reapply_leaves_the_skill_file_byte_stable() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill("stable", &[], "# Stable\n")]);
        apply(&HermesSync, &m, sb).expect("first apply");
        let path = sb.hermes_skills.join("stable").join("SKILL.md");
        let first = fs::read_to_string(&path).expect("first write");
        let mtime = fs::metadata(&path)
            .expect("meta")
            .modified()
            .expect("mtime");

        apply(&HermesSync, &m, sb).expect("second apply");
        assert_eq!(fs::read_to_string(&path).expect("second read"), first);
        assert_eq!(
            fs::metadata(&path)
                .expect("meta")
                .modified()
                .expect("mtime"),
            mtime,
            "an unchanged skill must not be rewritten"
        );
    });
}

#[test]
fn clearing_a_host_removes_the_managed_dirs_and_the_sidecar() {
    with_sandbox(|sb| {
        let m = manifest(vec![skill("managed", &[], "# Managed\n")]);
        apply(&HermesSync, &m, sb).expect("apply");
        let sidecar = sb.hermes_skills.join(".systemprompt-managed.json");
        assert!(sidecar.is_file(), "the sidecar records what we manage");

        let plugin_mcp_servers = BTreeMap::new();
        let client = stub_client();
        let ctx = HostSyncCtx {
            manifest: &m,
            org_plugins_root: sb.org_plugins.as_path(),
            plugin_mcp_servers: &plugin_mcp_servers,
            client: &client,
            bearer: "",
            loopback: &LOOPBACK,
            mcp_registry: &EMPTY_REGISTRY,
        };
        HermesSync.clear(&ctx).expect("clear");
        assert!(
            dirs_in(&sb.hermes_skills).is_empty(),
            "every managed dir is gone, got {:?}",
            dirs_in(&sb.hermes_skills)
        );
        assert!(!sidecar.exists(), "the sidecar goes with them");
    });
}
