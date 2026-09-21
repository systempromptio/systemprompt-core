#![cfg(unix)]

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use systemprompt_cli_integration_tests::full_bootstrap::{isolated_fixture, systemprompt_bin};
use systemprompt_cloud::tenants::{StoredTenant, TenantStore};
use systemprompt_identifiers::TenantId;
use systemprompt_test_fixtures::DisposableDb;

fn owned_pty() -> (File, File) {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both descriptors on success; null optional
    // outputs are allowed.
    let result = unsafe {
        nix::libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert_eq!(result, 0, "create owned pseudo-terminal");
    // SAFETY: each successful openpty descriptor is valid and transferred exactly
    // once.
    let master = File::from(unsafe { OwnedFd::from_raw_fd(master) });
    // SAFETY: each successful openpty descriptor is valid and transferred exactly
    // once.
    let slave = File::from(unsafe { OwnedFd::from_raw_fd(slave) });
    (master, slave)
}

const MAX_TRANSCRIPT_BYTES: usize = 1024 * 1024;

fn sanitize_output(output: &str, secrets: &[&str]) -> String {
    secrets.iter().fold(output.to_owned(), |safe, secret| {
        if secret.is_empty() {
            safe
        } else {
            safe.replace(secret, "***")
        }
    })
}

struct OwnedChild {
    child: Option<Child>,
    process_group: i32,
}

impl OwnedChild {
    fn new(child: Child) -> Self {
        Self {
            process_group: child.id() as i32,
            child: Some(child),
        }
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.as_mut().expect("owned child present").try_wait()
    }

    fn disarm(&mut self) {
        self.child.take();
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // SAFETY: the negative id targets only the process group created for this
            // child.
            unsafe {
                nix::libc::kill(-self.process_group, nix::libc::SIGKILL);
            }
            let _ = child.wait();
        }
    }
}

fn set_nonblocking(file: &File) {
    let fd = file.as_raw_fd();
    // SAFETY: fd is owned by file and remains open for both fcntl calls.
    let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFL) };
    assert!(flags >= 0, "read PTY status flags");
    // SAFETY: fd remains valid and F_SETFL accepts the retrieved flags plus
    // O_NONBLOCK.
    let result = unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK) };
    assert_eq!(result, 0, "make owned PTY nonblocking");
}

enum DrainStatus {
    Eof,
    Pending,
    Limit,
}

fn drain_pty(master: &mut File, transcript: &mut Vec<u8>) -> DrainStatus {
    let mut buffer = [0_u8; 4096];
    let mut drained = 0;
    loop {
        match master.read(&mut buffer) {
            Ok(0) => return DrainStatus::Eof,
            Ok(read) => {
                let remaining = MAX_TRANSCRIPT_BYTES.saturating_sub(transcript.len());
                transcript.extend_from_slice(&buffer[..read.min(remaining)]);
                if read > remaining {
                    return DrainStatus::Limit;
                }
                drained += read;
                if drained >= 64 * 1024 {
                    return DrainStatus::Pending;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return DrainStatus::Pending;
            },
            Err(error) if error.raw_os_error() == Some(nix::libc::EIO) => {
                return DrainStatus::Eof;
            },
            Err(error) => panic!("read owned PTY transcript: {error}"),
        }
    }
}

fn terminal_echo_disabled(master: &File) -> bool {
    let mut settings = std::mem::MaybeUninit::<nix::libc::termios>::uninit();
    // SAFETY: tcgetattr initializes settings for the valid owned PTY descriptor on
    // success.
    let result = unsafe { nix::libc::tcgetattr(master.as_raw_fd(), settings.as_mut_ptr()) };
    if result != 0 {
        return false;
    }
    // SAFETY: settings was initialized by the successful tcgetattr call.
    let settings = unsafe { settings.assume_init() };
    settings.c_lflag & nix::libc::ECHO == 0
}

fn run_interactive_profile_command(
    mut command: Command,
    database_url: &str,
    password: &str,
    additional_secrets: &[&str],
    interactions: &[(&str, &str, bool)],
) -> (ExitStatus, String) {
    let (mut master, slave) = owned_pty();
    set_nonblocking(&master);
    let stdout = slave.try_clone().expect("clone PTY stdout");
    let stderr = slave.try_clone().expect("clone PTY stderr");
    // SAFETY: the callback performs only setpgid and errno inspection before exec.
    unsafe {
        command.pre_exec(|| {
            if nix::libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let child = command
        .stdin(Stdio::from(slave))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn real profile authoring command");
    let mut child = OwnedChild::new(child);
    drop(command);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut transcript = Vec::new();
    let mut next_interaction = 0;
    let mut prompt_search_offset = 0;
    let mut redactions = vec![database_url, password];
    redactions.extend_from_slice(additional_secrets);

    let status = loop {
        if matches!(drain_pty(&mut master, &mut transcript), DrainStatus::Limit) {
            let safe = sanitize_output(&String::from_utf8_lossy(&transcript), &redactions);
            panic!("profile authoring transcript exceeded one MiB:\n{safe}");
        }
        if let Some((prompt, response, requires_hidden_input)) = interactions.get(next_interaction)
            && String::from_utf8_lossy(&transcript[prompt_search_offset..]).contains(prompt)
            && (!requires_hidden_input || terminal_echo_disabled(&master))
        {
            master
                .write_all(response.as_bytes())
                .expect("answer owned interactive profile prompt");
            master.flush().expect("flush interactive profile answer");
            next_interaction += 1;
            prompt_search_offset = transcript.len();
        }
        if let Some(status) = child.try_wait().expect("poll profile authoring child") {
            break status;
        }
        if Instant::now() >= deadline {
            let safe = sanitize_output(&String::from_utf8_lossy(&transcript), &redactions);
            panic!("profile authoring child exceeded 120 seconds:\n{safe}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    child.disarm();

    let drain_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match drain_pty(&mut master, &mut transcript) {
            DrainStatus::Eof => break,
            DrainStatus::Pending if Instant::now() < drain_deadline => {
                std::thread::sleep(Duration::from_millis(10));
            },
            DrainStatus::Pending => break,
            DrainStatus::Limit => {
                let safe = sanitize_output(&String::from_utf8_lossy(&transcript), &redactions);
                panic!("profile authoring transcript exceeded one MiB:\n{safe}");
            },
        }
    }
    let transcript = String::from_utf8_lossy(&transcript).into_owned();
    assert_eq!(
        next_interaction,
        interactions.len(),
        "not every expected interactive prompt was displayed:\n{}",
        sanitize_output(&transcript, &redactions)
    );
    (status, transcript)
}

#[tokio::test]
async fn profile_create_selects_the_owned_tenant_and_keeps_prompted_provider_secret_hidden() {
    let database = DisposableDb::create("cli_profile_interactive")
        .await
        .expect("create isolated interactive profile database");
    let parsed = url::Url::parse(database.url()).expect("fixture database URL");
    let password = parsed.password().expect("fixture database password");
    let fixture = isolated_fixture(8080);
    let project = fixture
        .profile_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("fixture project root");
    let rejected_id = TenantId::new("tenant-not-selected");
    let selected_id = TenantId::new("tenant-interactive-selected");
    let tenants = vec![
        StoredTenant::new_local(
            rejected_id,
            "First Tenant Must Not Be Selected".to_owned(),
            "postgres://unused:unused@127.0.0.1:1/unused".to_owned(),
        ),
        StoredTenant::new_local(
            selected_id.clone(),
            "Owned Interactive Database".to_owned(),
            database.url().to_owned(),
        ),
    ];
    let tenant_path = project.join(".systemprompt/tenants.json");
    std::fs::create_dir_all(tenant_path.parent().expect("tenant store parent"))
        .expect("create tenant store directory");
    TenantStore::new(tenants)
        .save_to_path(&tenant_path)
        .expect("write interactive tenant choices");

    let profile_name = "profile_interactive";
    let prompted_key = "synthetic-hidden-interactive-provider-key";
    let mut command = Command::new(systemprompt_bin());
    command
        .current_dir(project)
        .env("HOME", project)
        .env("DATABASE_URL", database.url())
        .env_remove("INTERNAL_DATABASE_URL")
        .env_remove("RUST_LOG")
        .env_remove("SYSTEMPROMPT_PROFILE")
        .env_remove("SYSTEMPROMPT_NON_INTERACTIVE")
        .env_remove("SYSTEMPROMPT_OUTPUT_FORMAT")
        .env_remove("SYSTEMPROMPT_SERVICES_PATH")
        .env_remove("SYSTEMPROMPT_CLI_REMOTE")
        .env_remove("SYSTEMPROMPT_DEPLOYMENT_HOST")
        .env_remove("FLY_APP_NAME")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .arg("--no-color")
        .args(["cloud", "profile", "create", profile_name]);
    let (status, transcript) = run_interactive_profile_command(
        command,
        database.url(),
        password,
        &[prompted_key],
        &[
            ("Profile type", "\r", false),
            ("Select tenant", "\x1b[B\r", false),
            ("Select your AI provider", "\x1b[B\x1b[B\r", false),
            ("OpenAI API Key", &format!("{prompted_key}\r"), true),
            ("Run database migrations?", "n\r", false),
        ],
    );
    let safe_transcript = sanitize_output(&transcript, &[database.url(), password, prompted_key]);
    assert!(
        status.success(),
        "interactive profile creation failed:\n{safe_transcript}"
    );
    assert!(
        !transcript.contains(prompted_key),
        "password prompt echoed the provider credential"
    );

    let created = project.join(".systemprompt/profiles").join(profile_name);
    let profile: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(created.join("profile.yaml")).expect("read interactive profile"),
    )
    .expect("interactive profile YAML");
    assert_eq!(profile["cloud"]["tenant_id"], selected_id.as_str());
    let secrets: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(created.join("secrets.json")).expect("read interactive secrets"),
    )
    .expect("interactive secrets JSON");
    assert!(
        secrets["database_url"].as_str() == Some(database.url()),
        "interactive selection persisted the wrong tenant database"
    );
    assert!(
        secrets["openai"].as_str() == Some(prompted_key),
        "hidden provider credential was not persisted"
    );
    assert!(secrets["anthropic"].is_null());
    assert!(secrets["gemini"].is_null());

    let pool = database.pool().await.expect("interactive database pool");
    let raw = pool.pool_arc().expect("raw interactive database pool");
    let migration_table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.extension_migrations')::text")
            .fetch_one(raw.as_ref())
            .await
            .expect("inspect migration cancellation state");
    assert_eq!(
        migration_table, None,
        "declined migrations must not alter the database"
    );
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
#[tokio::test]
async fn profile_edit_applies_real_interactive_server_security_runtime_and_secret_inputs() {
    let fixture = isolated_fixture(8080);
    let project = fixture
        .profile_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("fixture project root");
    let profile_name = "covfix";
    let editable_profile_dir = project.join(".systemprompt/profiles").join(profile_name);
    std::fs::create_dir_all(&editable_profile_dir).expect("create editable profile directory");
    std::fs::copy(
        &fixture.profile_path,
        editable_profile_dir.join("profile.yaml"),
    )
    .expect("copy editable profile");
    let secrets_path = editable_profile_dir.join("secrets.json");
    std::fs::write(
        &secrets_path,
        r#"{"database_url":"postgres://original.invalid/db","openai":"existing-hidden-key"}"#,
    )
    .expect("seed editable secrets");

    let mut command = Command::new(systemprompt_bin());
    command
        .current_dir(project)
        .env("HOME", project)
        .env_remove("DATABASE_URL")
        .env_remove("INTERNAL_DATABASE_URL")
        .env_remove("RUST_LOG")
        .env_remove("SYSTEMPROMPT_PROFILE")
        .env_remove("SYSTEMPROMPT_NON_INTERACTIVE")
        .env_remove("SYSTEMPROMPT_OUTPUT_FORMAT")
        .env_remove("SYSTEMPROMPT_SERVICES_PATH")
        .env_remove("SYSTEMPROMPT_CLI_REMOTE")
        .env_remove("SYSTEMPROMPT_DEPLOYMENT_HOST")
        .env_remove("FLY_APP_NAME")
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .arg("--no-color")
        .args(["cloud", "profile", "edit", profile_name]);
    let (status, transcript) = run_interactive_profile_command(
        command,
        "synthetic-not-present",
        "synthetic-not-present",
        &["existing-hidden-key", "postgres://edited.invalid/db"],
        &[
            ("What would you like to edit?", "\r", false),
            ("Host", "0.0.0.0\r", false),
            ("Port", "9099\r", false),
            ("API Server URL", "http://127.0.0.1:9099\r", false),
            (
                "API External URL",
                "https://edited.example.invalid\r",
                false,
            ),
            ("Use HTTPS?", "y", false),
            ("What would you like to edit?", "\x1b[B\r", false),
            ("JWT Issuer", "https://issuer.edited.invalid\r", false),
            ("Access Token Expiration", "\r", false),
            ("Refresh Token Expiration", "\r", false),
            ("What would you like to edit?", "\x1b[B\x1b[B\r", false),
            ("Environment", "\x1b[B\x1b[B\r", false),
            ("Log Level", "\x1b[B\x1b[B\x1b[B\r", false),
            (
                "What would you like to edit?",
                "\x1b[B\x1b[B\x1b[B\r",
                false,
            ),
            ("Select key to edit", "\x1b[B\x1b[B\x1b[B\r", false),
            ("New Database URL", "postgres://edited.invalid/db\r", false),
            ("Select key to edit", "\x1b[B\x1b[B\x1b[B\x1b[B\r", false),
            (
                "What would you like to edit?",
                "\x1b[B\x1b[B\x1b[B\x1b[B\r",
                false,
            ),
        ],
    );
    let safe_transcript = sanitize_output(
        &transcript,
        &["existing-hidden-key", "postgres://edited.invalid/db"],
    );
    assert!(
        status.success(),
        "interactive profile edit failed:\n{safe_transcript}"
    );
    assert!(
        !transcript.contains("existing-hidden-key"),
        "existing provider secret leaked into the prompt transcript"
    );

    let profile: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(editable_profile_dir.join("profile.yaml"))
            .expect("read edited profile"),
    )
    .expect("edited profile YAML");
    assert_eq!(profile["server"]["host"], "0.0.0.0");
    assert_eq!(profile["server"]["port"], 9099);
    assert_eq!(profile["server"]["api_server_url"], "http://127.0.0.1:9099");
    assert_eq!(
        profile["server"]["api_external_url"],
        "https://edited.example.invalid"
    );
    assert_eq!(profile["server"]["use_https"], true);
    assert_eq!(
        profile["security"]["jwt_issuer"],
        "https://issuer.edited.invalid"
    );
    assert_eq!(profile["runtime"]["environment"], "staging");
    assert_eq!(profile["runtime"]["log_level"], "debug");
    let secrets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(secrets_path).expect("read edited secrets"))
            .expect("edited secrets JSON");
    assert_eq!(secrets["database_url"], "postgres://edited.invalid/db");
    assert_eq!(secrets["openai"], "existing-hidden-key");
}
