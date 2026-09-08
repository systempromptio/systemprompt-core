use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::session::{SessionCommands, execute};
use systemprompt_cli::session::{
    clear_all_sessions, clear_session, get_or_create_session, load_session_store,
};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::CloudCredentials;
use systemprompt_identifiers::{CloudAuthToken, Email, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_db_pool, install_test_signing_key, seed_user_row_with_roles,
};

struct Project {
    previous: PathBuf,
    _root: tempfile::TempDir,
    profile: PathBuf,
}

impl Project {
    fn new(username: &str, tenant: bool) -> Self {
        let boot = ensure_test_bootstrap();
        let root = tempfile::tempdir().unwrap();
        let profile = root
            .path()
            .join(".systemprompt/profiles/coverage/profile.yaml");
        std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
        let mut yaml = std::fs::read_to_string(&boot.profile_path)
            .unwrap()
            .replace("username: testadmin", &format!("username: {username}"));
        if tenant {
            yaml.push_str("\ncloud:\n  tenant_id: coverage-tenant\n  validation: strict\n");
        }
        std::fs::write(&profile, yaml).unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        Self {
            previous,
            _root: root,
            profile,
        }
    }

    fn context(&self, interactive: bool) -> CommandContext {
        CommandContext::new(
            CliConfig::new()
                .with_interactive(interactive)
                .with_output_format(OutputFormat::Json)
                .with_profile_override(Some(self.profile.display().to_string())),
            EnvOverrides::default(),
        )
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
    }
}

async fn admin() -> UserId {
    let boot = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let id = UserId::new(format!("sessionflow-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row_with_roles(
        &pool,
        &id,
        &format!("{}@example.invalid", id.as_str()),
        &["admin".to_owned()],
    )
    .await
    .unwrap();
    id
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: SessionCommands,
}

async fn command(project: &Project, args: &[&str]) -> anyhow::Result<()> {
    let args =
        Args::try_parse_from(std::iter::once("session").chain(args.iter().copied())).unwrap();
    execute(args.command, &project.context(false)).await
}

#[tokio::test]
async fn coverage_local_session_creation_persists_identity_and_reuses_the_session() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    let ctx = project.context(true);
    let first = get_or_create_session(&ctx).await.unwrap().session;
    assert_eq!(first.user_id, user);
    assert!(!first.session_token.as_str().is_empty());
    assert_eq!(first.profile_name.as_str(), "coverage");
    let second = get_or_create_session(&ctx).await.unwrap().session;
    assert_eq!(second.session_id, first.session_id);
    assert_eq!(second.context_id, first.context_id);
    let stored = load_session_store().unwrap();
    assert_eq!(stored.active_profile_name.as_deref(), Some("coverage"));
    let active = stored.active_session_for_profile_discovery().unwrap();
    assert_eq!(active.user_id, user);
    assert_eq!(active.session_id, first.session_id);
}

#[tokio::test]
async fn coverage_active_profile_is_resolved_after_explicit_override_is_removed() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    let first = get_or_create_session(&project.context(false))
        .await
        .unwrap()
        .session;
    let ctx = CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    );
    let resumed = get_or_create_session(&ctx).await.unwrap().session;
    assert_eq!(resumed.session_id, first.session_id);
    assert_eq!(resumed.user_id, user);
}

#[tokio::test]
async fn coverage_logout_removes_active_and_explicit_profile_sessions() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    for args in [
        &["logout", "--yes"][..],
        &["logout", "--profile", "coverage", "--yes"][..],
        &["logout", "--all", "--yes"][..],
    ] {
        get_or_create_session(&project.context(false))
            .await
            .unwrap();
        assert!(!load_session_store().unwrap().is_empty());
        command(&project, args).await.unwrap();
        assert!(load_session_store().unwrap().is_empty());
    }
    command(&project, &["logout", "--yes"]).await.unwrap();
    assert!(load_session_store().unwrap().is_empty());
}

#[tokio::test]
async fn coverage_clearing_sessions_is_persisted_and_idempotent() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    clear_session().unwrap();
    assert!(load_session_store().unwrap().is_empty());
    clear_session().unwrap();
    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    clear_all_sessions().unwrap();
    assert!(load_session_store().unwrap().is_empty());
}

#[tokio::test]
async fn coverage_tenant_session_uses_cloud_identity_and_tenant_binding() {
    let user = admin().await;
    let project = Project::new(user.as_str(), true);
    let email = format!("{}@example.invalid", user.as_str());
    let credentials = CloudCredentials::new(
        CloudAuthToken::new("eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.fixture"),
        "https://example.invalid".to_owned(),
        Email::new(email),
    );
    credentials
        .save_to_path(
            &systemprompt_cloud::paths::get_cloud_paths()
                .resolve(systemprompt_cloud::paths::CloudPath::Credentials),
        )
        .unwrap();
    let resolved = get_or_create_session(&project.context(true))
        .await
        .unwrap()
        .session;
    assert_eq!(resolved.user_id, user);
    assert_eq!(
        resolved.tenant_key.as_ref().unwrap().as_str(),
        "coverage-tenant"
    );
    assert_eq!(resolved.profile_name.as_str(), "coverage");
    assert!(!resolved.session_token.as_str().is_empty());
    assert_eq!(
        get_or_create_session(&project.context(false))
            .await
            .unwrap()
            .session
            .session_id,
        resolved.session_id
    );
}

#[test]
fn coverage_manifest_plugins_are_listed_by_type_and_shown_case_insensitively() {
    use systemprompt_cli::plugins::{list, show};
    let _project = Project::new("unused", false);
    for (name, kind) in [("coverage-cli", "cli"), ("coverage-mcp", "mcp")] {
        let dir = std::path::Path::new("extensions").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.yaml"),
            format!(
                "extension:\n  type: {kind}\n  name: {name}\n  binary: {name}\n  enabled: true\n"
            ),
        )
        .unwrap();
    }
    let config = CliConfig::new().with_interactive(false);
    for (kind, cli, mcp) in [
        ("manifest", true, true),
        ("cli", true, false),
        ("mcp", false, true),
        ("compiled", false, false),
    ] {
        let out = list::execute(
            &list::ListArgs {
                filter: None,
                capability: None,
                r#type: kind.to_owned(),
            },
            &config,
        );
        let text = serde_json::to_string(out.artifact()).unwrap();
        assert_eq!(text.contains("coverage-cli"), cli, "{kind}: {text}");
        assert_eq!(text.contains("coverage-mcp"), mcp, "{kind}: {text}");
    }
    let out = list::execute(
        &list::ListArgs {
            filter: Some("COVERAGE-MCP".to_owned()),
            capability: None,
            r#type: "manifest".to_owned(),
        },
        &config,
    );
    let text = serde_json::to_string(out.artifact()).unwrap();
    assert!(text.contains("coverage-mcp") && !text.contains("coverage-cli"));
    let out = show::execute(
        &show::ShowArgs {
            id: "COVERAGE-MCP".to_owned(),
        },
        &config,
    )
    .unwrap();
    let text = serde_json::to_string(out.artifact()).unwrap();
    assert!(text.contains("coverage-mcp") && text.contains("manifest"));
    assert!(
        show::execute(
            &show::ShowArgs {
                id: "unknown-manifest".to_owned()
            },
            &config
        )
        .is_err()
    );
}

#[test]
fn coverage_deploy_selection_excludes_local_profiles_and_resolves_named_profiles() {
    use systemprompt_cli::ScriptedPrompter;
    use systemprompt_cli::cloud::deploy::select::resolve_profile;
    let project = Project::new("unused", true);
    let config = CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true);
    let err = resolve_profile(&ScriptedPrompter::default(), None, &config).unwrap_err();
    assert!(
        err.to_string().contains("No deployable profiles"),
        "{err:#}"
    );
    let yaml = std::fs::read_to_string(&project.profile)
        .unwrap()
        .replace("target: local", "target: cloud");
    std::fs::write(&project.profile, yaml).unwrap();
    let (selected, path) = resolve_profile(&ScriptedPrompter::new(["0"]), None, &config).unwrap();
    assert_eq!(path, project.profile);
    assert_eq!(
        selected.cloud.unwrap().tenant_id.unwrap().as_str(),
        "coverage-tenant"
    );
    let (_, explicit) =
        resolve_profile(&ScriptedPrompter::default(), Some("coverage"), &config).unwrap();
    assert_eq!(explicit, project.profile);
    assert!(
        resolve_profile(&ScriptedPrompter::default(), Some("missing"), &config)
            .unwrap_err()
            .to_string()
            .contains("not found")
    );
    let noninteractive = CliConfig::new().with_interactive(false);
    assert!(resolve_profile(&ScriptedPrompter::default(), None, &noninteractive).is_err());
}

fn profile_context(project: &Project, answers: &[&str]) -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
        EnvOverrides {
            profile: Some(project.profile.display().to_string()),
            ..Default::default()
        },
    )
    .with_prompter(Box::new(systemprompt_cli::ScriptedPrompter::new(
        answers.iter().copied(),
    )))
}

#[tokio::test]
async fn coverage_profile_menu_edits_an_existing_profile_and_returns_to_the_menu() {
    let project = Project::new("unused", false);
    let before = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    systemprompt_cli::cloud::profile::execute(None, &profile_context(&project, &["1", "4", "3"]))
        .await
        .unwrap();
    let after = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    assert_eq!(after.server.host, before.server.host);
    assert_eq!(after.server.port, before.server.port);
}

#[tokio::test]
async fn coverage_profile_menu_declining_deletion_preserves_profile_and_secrets() {
    let project = Project::new("unused", false);
    let secret = project.profile.parent().unwrap().join("secrets.json");
    std::fs::write(&secret, r#"{"custom":"keep"}"#).unwrap();
    systemprompt_cli::cloud::profile::execute(
        None,
        &profile_context(&project, &["2", "0", "no", "3"]),
    )
    .await
    .unwrap();
    assert!(project.profile.exists());
    assert_eq!(
        std::fs::read_to_string(secret).unwrap(),
        r#"{"custom":"keep"}"#
    );
}

#[tokio::test]
async fn coverage_profile_menu_confirmed_deletion_removes_only_selected_profile() {
    let project = Project::new("unused", false);
    let unrelated = project
        ._root
        .path()
        .join(".systemprompt/profiles/not-a-profile");
    std::fs::create_dir(&unrelated).unwrap();
    std::fs::write(unrelated.join("keep.txt"), "keep").unwrap();
    systemprompt_cli::cloud::profile::execute(
        None,
        &profile_context(&project, &["2", "0", "yes", "3"]),
    )
    .await
    .unwrap();
    assert!(!project.profile.parent().unwrap().exists());
    assert!(unrelated.join("keep.txt").exists());
}

#[tokio::test]
async fn coverage_profile_edit_persists_server_security_and_runtime_choices() {
    let project = Project::new("unused", false);
    systemprompt_cli::cloud::profile::execute(
        None,
        &profile_context(
            &project,
            &[
                "1",
                "0",
                "127.0.0.2",
                "8123",
                "http://127.0.0.2:8123",
                "https://fixture.example",
                "yes",
                "1",
                "https://issuer.example",
                "600",
                "1200",
                "2",
                "2",
                "3",
                "4",
                "3",
            ],
        ),
    )
    .await
    .unwrap();
    let after = systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap();
    assert_eq!(after.server.host, "127.0.0.2");
    assert_eq!(after.server.port, 8123);
    assert!(after.server.use_https);
    assert_eq!(after.security.issuer, "https://issuer.example");
    assert_eq!(after.security.access_token_expiration, 600);
    assert_eq!(after.security.refresh_token_expiration, 1200);
    assert_eq!(after.runtime.environment.to_string(), "staging");
}
