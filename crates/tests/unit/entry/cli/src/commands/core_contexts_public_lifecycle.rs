//! Public context command lifecycle with durable session switching.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore};
use systemprompt_identifiers::{Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_loader::ProfileLoader;
use systemprompt_models::auth::UserType;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context_with, install_test_signing_key,
    seed_user_row, seed_user_session,
};

const HELPER: &str = "commands::core_contexts_public_lifecycle::public_context_lifecycle_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .expect("parse core command")
        .command
}

struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

async fn run_marked(label: &str, args: &[&str], context: &CommandContext) {
    println!("BEGIN_{label}");
    core::execute(parse(args), context)
        .await
        .unwrap_or_else(|error| panic!("{label} failed: {error:#}"));
    println!("END_{label}");
}

#[tokio::test]
#[ignore = "re-executed by public_context_commands_switch_persist_and_protect_the_active_context"]
async fn public_context_lifecycle_helper() {
    let database = DisposableDb::installed("cli_public_contexts")
        .await
        .expect("private contexts database");
    // SAFETY: the ignored helper is process-isolated and configuration has not been
    // initialized.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    let boot = ensure_test_bootstrap();
    install_test_signing_key();
    let project = tempfile::tempdir().expect("owned context project");
    let profile_path = project
        .path()
        .join(".systemprompt/profiles/coverage/profile.yaml");
    std::fs::create_dir_all(profile_path.parent().expect("profile directory"))
        .expect("create profile directory");
    std::fs::copy(&boot.profile_path, &profile_path).expect("copy owned profile");
    let previous = std::env::current_dir().expect("current directory");
    std::env::set_current_dir(project.path()).expect("enter owned project");
    let _cwd = CwdGuard(previous);

    let pool = database.pool().await.expect("private contexts pool");
    let user = UserId::new(format!("ctx-public-{}", uuid::Uuid::new_v4().simple()));
    let session_id = SessionId::generate();
    seed_user_row(&pool, &user, "contexts-public@example.invalid")
        .await
        .expect("seed context owner");
    seed_user_session(&pool, &user, &session_id)
        .await
        .expect("seed CLI session row");
    let repository = ContextRepository::new(&pool).expect("context repository");
    let initial_context = repository
        .create_context(
            &user,
            Some(&session_id),
            "Initial Context",
            ContextKind::User,
        )
        .await
        .expect("seed initial active context");
    let profile = ProfileLoader::load_from_path(&profile_path).expect("load owned profile");
    let session = CliSession::builder(
        SessionBinding::new(
            ProfileName::try_new("coverage").expect("valid profile name"),
            profile.security.issuer.clone(),
        ),
        SessionToken::new("owned-context-session-token"),
        session_id,
        initial_context,
        SessionIdentity::new(
            user.clone(),
            Email::try_new("contexts-public@example.invalid").expect("fixture email"),
            UserType::Admin,
        ),
    )
    .with_profile_path(&profile_path)
    .build();
    let sessions_dir = ResolvedPaths::discover().sessions_dir();
    let mut store = SessionStore::load_or_create(&sessions_dir).expect("session store");
    store.upsert_session(&SessionKey::Local, session);
    store.set_active_with_profile(&SessionKey::Local, "coverage");
    store
        .save(&sessions_dir)
        .expect("persist active CLI session");

    let paths = PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    };
    let app = fixture_app_context_with(
        &pool,
        database.url(),
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .expect("full context app");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json)
            .with_profile_override(Some(profile_path.display().to_string())),
        EnvOverrides::default(),
        app,
    );

    run_marked(
        "CONTEXT_CREATE",
        &["contexts", "create", "--name", "Created Context"],
        &context,
    )
    .await;
    run_marked(
        "CONTEXT_EDIT",
        &[
            "contexts",
            "edit",
            "Created Context",
            "--name",
            "Renamed Context",
        ],
        &context,
    )
    .await;
    run_marked(
        "CONTEXT_USE",
        &["contexts", "use", "Renamed Context"],
        &context,
    )
    .await;
    run_marked(
        "CONTEXT_NEW",
        &["contexts", "new", "--name", "Active New Context"],
        &context,
    )
    .await;
    let active_delete = core::execute(
        parse(&["contexts", "delete", "Active New Context", "--yes"]),
        &context,
    )
    .await
    .expect_err("active context deletion must be refused");
    println!(
        "ACTIVE_DELETE_ERROR={}",
        serde_json::json!(format!("{active_delete:#}"))
    );
    run_marked(
        "CONTEXT_DELETE",
        &["contexts", "delete", "Renamed Context", "--yes"],
        &context,
    )
    .await;

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT context_id, name FROM user_contexts WHERE user_id = $1 ORDER BY name",
    )
    .bind(user.as_str())
    .fetch_all(pool.pool_arc().expect("private SQL pool").as_ref())
    .await
    .expect("read durable contexts");
    let stored = SessionStore::load_or_create(&sessions_dir).expect("reload session store");
    let active = stored
        .get_session(&SessionKey::Local)
        .expect("stored local session")
        .context_id
        .as_str()
        .to_owned();
    println!(
        "CONTEXT_STATE={}",
        serde_json::json!({"rows": rows, "active_context": active})
    );

    drop(context);
    drop(repository);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let mut stdout = tempfile::NamedTempFile::new().expect("contexts stdout");
    let mut stderr = tempfile::NamedTempFile::new().expect("contexts stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn contexts helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll contexts helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait().expect("reap contexts helper");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |file: &mut tempfile::NamedTempFile| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read capture");
        bytes
    };
    Output {
        status,
        stdout: read(&mut stdout),
        stderr: read(&mut stderr),
    }
}

fn marked_json(stdout: &str, label: &str) -> serde_json::Value {
    let begin = format!("BEGIN_{label}\n");
    let end = format!("\nEND_{label}");
    let document = stdout
        .split_once(&begin)
        .and_then(|(_, tail)| tail.split_once(&end))
        .map(|(json, _)| json)
        .unwrap_or_else(|| panic!("missing {label} output markers"));
    serde_json::from_str(document).unwrap_or_else(|_| panic!("invalid {label} JSON artifact"))
}

#[test]
fn public_context_commands_switch_persist_and_protect_the_active_context() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(output.status.success(), "public contexts helper failed");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 contexts output");
    for (label, title) in [
        ("CONTEXT_CREATE", "Context Created"),
        ("CONTEXT_EDIT", "Context Updated"),
        ("CONTEXT_USE", "Context Switched"),
        ("CONTEXT_NEW", "New Context Created"),
        ("CONTEXT_DELETE", "Context Deleted"),
    ] {
        assert_eq!(marked_json(&stdout, label)["title"], title, "{label}");
    }
    let error = stdout
        .split_once("ACTIVE_DELETE_ERROR=")
        .map(|(_, tail)| tail.lines().next().expect("active-delete line"))
        .expect("active-delete marker");
    let error: String = serde_json::from_str(error).expect("active-delete JSON string");
    assert!(
        error.contains("Cannot delete the active context"),
        "{error}"
    );
    let state = stdout
        .split_once("CONTEXT_STATE=")
        .map(|(_, tail)| tail.lines().next().expect("context-state line"))
        .expect("context-state marker");
    let state: serde_json::Value = serde_json::from_str(state).expect("context-state JSON");
    let rows = state["rows"].as_array().expect("context rows");
    assert_eq!(rows.len(), 2, "{state}");
    assert_eq!(rows[0][1], "Active New Context", "{state}");
    assert_eq!(rows[1][1], "Initial Context", "{state}");
    assert_eq!(state["active_context"], rows[0][0], "{state}");
}
