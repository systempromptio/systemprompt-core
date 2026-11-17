use clap::Parser;
use systemprompt_cli::admin::session::{SessionCommands, execute};
use systemprompt_cli::session::{
    clear_all_sessions, clear_session, get_or_create_session, load_session_store,
};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_cloud::{CloudCredentials, SessionKey};
use systemprompt_extension::cli::CliConfig;
use systemprompt_identifiers::{CloudAuthToken, Email, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row_with_roles, test_db_pool};

use crate::session_project_fixture::Project;

async fn admin() -> UserId {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
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
    execute(args.command, &project.context(false))
        .await
        .map(drop)
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
    let stored = load_session_store(&project.paths()).unwrap();
    assert_eq!(
        stored.active_profile_name, None,
        "an explicit --profile is a one-shot target and never becomes the active session"
    );
    assert!(stored.active_key.is_none());
    let kept = stored.get_session(&SessionKey::Local).unwrap();
    assert_eq!(kept.user_id, user);
    assert_eq!(kept.session_id, first.session_id);
}

#[tokio::test]
async fn coverage_active_profile_is_resolved_after_explicit_override_is_removed() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    let first = get_or_create_session(&project.context(false))
        .await
        .unwrap()
        .session;
    let ctx = crate::kernel_host::kernel_host(
        CommandContext::new(
            CliConfig::new().with_interactive(false),
            EnvOverrides::default(),
        )
        .with_profile(Some(
            systemprompt_config::LoadedProfile::from_path(&project.profile).unwrap(),
        )),
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
        &["logout", "--profile", "coverage", "--yes"][..],
        &["logout", "--all", "--yes"][..],
    ] {
        get_or_create_session(&project.context(false))
            .await
            .unwrap();
        assert!(!load_session_store(&project.paths()).unwrap().is_empty());
        command(&project, args).await.unwrap();
        assert!(load_session_store(&project.paths()).unwrap().is_empty());
    }

    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    let err = command(&project, &["logout", "--yes"])
        .await
        .expect_err("a session minted under --profile is not the active session");
    assert!(format!("{err:#}").contains("No active session"), "{err:#}");
    assert!(!load_session_store(&project.paths()).unwrap().is_empty());
    command(&project, &["logout", "--all", "--yes"])
        .await
        .unwrap();
    assert!(load_session_store(&project.paths()).unwrap().is_empty());
}

#[tokio::test]
async fn coverage_clearing_sessions_is_persisted_and_idempotent() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    clear_session(
        &project.paths(),
        &systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap(),
    )
    .unwrap();
    assert!(load_session_store(&project.paths()).unwrap().is_empty());
    clear_session(
        &project.paths(),
        &systemprompt_loader::ProfileLoader::load_from_path(&project.profile).unwrap(),
    )
    .unwrap();
    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    clear_all_sessions(&project.paths()).unwrap();
    assert!(load_session_store(&project.paths()).unwrap().is_empty());
}

#[tokio::test]
async fn coverage_tenant_session_uses_cloud_identity_and_tenant_binding() {
    let user = admin().await;
    let project = Project::new(user.as_str(), true);
    let email = format!("{}@example.invalid", user.as_str());
    let credentials = CloudCredentials::new(
        CloudAuthToken::new("eyJhbGciOiJIUzI1NiJ9.eyJleHAiOjQxMDI0NDQ4MDB9.fixture"),
        "https://example.invalid".to_owned(),
        Email::try_new(email).expect("valid Email"),
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
    let host = CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    )
    .with_admitted(Some(systemprompt_test_fixtures::fixture_admitted()));
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
            &host,
        )
        .unwrap();
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
        &host,
    )
    .unwrap();
    let text = serde_json::to_string(out.artifact()).unwrap();
    assert!(text.contains("coverage-mcp") && !text.contains("coverage-cli"));
    let out = show::execute(
        &show::ShowArgs {
            id: "COVERAGE-MCP".to_owned(),
        },
        &host,
    )
    .unwrap();
    let text = serde_json::to_string(out.artifact()).unwrap();
    assert!(text.contains("coverage-mcp") && text.contains("manifest"));
    assert!(
        show::execute(
            &show::ShowArgs {
                id: "unknown-manifest".to_owned()
            },
            &host
        )
        .is_err()
    );
}

const LOGOUT_CONFIRM_HELPER: &str = "session_lifecycle_flows::logout_confirmation_helper";

#[tokio::test]
#[ignore = "re-executed by logout_confirmation_preserves_sessions_and_reports_cancellation"]
async fn logout_confirmation_helper() {
    let user = admin().await;
    let project = Project::new(user.as_str(), false);
    get_or_create_session(&project.context(false))
        .await
        .unwrap();
    let before = load_session_store(&project.paths()).unwrap().len();

    let mut single = project
        .context(true)
        .with_prompter(Box::new(systemprompt_cli::ScriptedPrompter::new(["no"])));
    single.cli = single.cli.clone().with_assume_terminal(true);
    println!("BEGIN_SINGLE_CANCEL");
    let parsed = Args::try_parse_from(["session", "logout", "--profile", "coverage"]).unwrap();
    crate::boot_ctx::rendered(&single, execute(parsed.command, &single).await).unwrap();
    println!("END_SINGLE_CANCEL");
    assert_eq!(load_session_store(&project.paths()).unwrap().len(), before);

    let mut all = project
        .context(true)
        .with_prompter(Box::new(systemprompt_cli::ScriptedPrompter::new(["no"])));
    all.cli = all.cli.clone().with_assume_terminal(true);
    println!("BEGIN_ALL_CANCEL");
    let parsed = Args::try_parse_from(["session", "logout", "--all"]).unwrap();
    crate::boot_ctx::rendered(&all, execute(parsed.command, &all).await).unwrap();
    println!("END_ALL_CANCEL");
    assert_eq!(load_session_store(&project.paths()).unwrap().len(), before);

    let parsed = Args::try_parse_from(["session", "logout", "--all"]).unwrap();
    let error = execute(parsed.command, &project.context(false))
        .await
        .expect_err("non-interactive --all requires explicit confirmation");
    println!("NONINTERACTIVE_ERROR={error:#}");
    assert_eq!(load_session_store(&project.paths()).unwrap().len(), before);
}

#[test]
fn logout_confirmation_preserves_sessions_and_reports_cancellation() {
    let output =
        std::process::Command::new(std::env::current_exe().expect("unit-test binary path"))
            .args(["--exact", LOGOUT_CONFIRM_HELPER, "--ignored", "--nocapture"])
            .output()
            .expect("re-execute logout confirmation helper");

    assert!(
        output.status.success(),
        "logout helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("logout output is UTF-8");
    for (begin, end, target) in [
        ("BEGIN_SINGLE_CANCEL", "END_SINGLE_CANCEL", "local"),
        ("BEGIN_ALL_CANCEL", "END_ALL_CANCEL", "all"),
    ] {
        let section = stdout
            .split_once(begin)
            .and_then(|(_, tail)| tail.split_once(end))
            .map(|(section, _)| section)
            .unwrap_or_else(|| panic!("missing {begin}/{end} in {stdout}"));
        assert!(section.contains("cancelled"), "{section}");
        assert!(section.contains("Operation cancelled"), "{section}");
        assert!(section.contains(target), "{section}");
    }
    assert!(
        stdout.contains("NONINTERACTIVE_ERROR=--yes is required in non-interactive mode for --all"),
        "{stdout}"
    );
}

#[tokio::test]
async fn malformed_active_profile_fails_without_replacing_the_bound_session() {
    use chrono::Duration;
    use systemprompt_cloud::paths::ResolvedPaths;
    use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity, SessionStore};
    use systemprompt_identifiers::{ContextId, ProfileName, SessionId, SessionToken};
    use systemprompt_models::auth::UserType;

    let project = Project::new("unused", false);
    let malformed = project.profile.parent().unwrap().join("malformed.yaml");
    std::fs::write(&malformed, "name: incomplete\ntarget: local\n")
        .expect("write malformed owned profile");
    let key = SessionKey::Local;
    let session = CliSession::builder(
        SessionBinding::new(
            ProfileName::try_new("malformed").expect("valid profile name"),
            "https://issuer.invalid".to_owned(),
        ),
        SessionToken::new("preserved-token"),
        SessionId::new("preserved-session"),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("preserved-user"),
            Email::try_new("preserved@example.invalid").expect("valid email"),
            UserType::Admin,
        ),
    )
    .with_session_key(&key)
    .with_profile_path(&malformed)
    .with_ttl(Duration::hours(1))
    .build();
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store = SessionStore::new();
    store.upsert_session(&key, session);
    store.set_active_with_profile(&key, &pname("malformed"));
    store
        .save(&sessions_dir)
        .expect("persist malformed-profile session");
    let session_file = sessions_dir.join("index.json");
    let before = std::fs::read(&session_file).expect("stored session bytes");
    let ctx = crate::kernel_host::kernel_host(CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    ));

    let error = get_or_create_session(&ctx)
        .await
        .expect_err("a malformed bound profile must stop session resolution");
    assert!(
        format!("{error:#}").contains("Failed to load profile from stored path"),
        "the error must identify the persisted profile boundary: {error:#}"
    );
    assert_eq!(
        std::fs::read(session_file).expect("store after malformed profile"),
        before,
        "profile parse failure must preserve the bound session for repair"
    );
}

fn pname(name: &str) -> systemprompt_identifiers::ProfileName {
    systemprompt_identifiers::ProfileName::try_new(name).expect("valid ProfileName")
}
