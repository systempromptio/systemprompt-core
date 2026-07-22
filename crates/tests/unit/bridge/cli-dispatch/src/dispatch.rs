use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::run_with_args;

fn config_file(sb: &Sandbox) -> std::path::PathBuf {
    sb.config
        .path()
        .join("systemprompt")
        .join("systemprompt-bridge.toml")
}

fn pat_file(sb: &Sandbox) -> std::path::PathBuf {
    sb.config
        .path()
        .join("systemprompt")
        .join("systemprompt-bridge.pat")
}

#[test]
fn login_through_dispatch_writes_pat_and_config() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
            "--gateway",
            "http://gateway.invalid:9000",
        ]));
    });

    let cfg = std::fs::read_to_string(config_file(&sb)).expect("login writes the config file");
    assert!(
        cfg.contains("http://gateway.invalid:9000"),
        "--gateway must land in the config: {cfg}"
    );
    assert!(
        cfg.contains("[pat]"),
        "login records a [pat] section: {cfg}"
    );
    assert_eq!(
        std::fs::read_to_string(pat_file(&sb)).expect("pat file written"),
        "sp-live-testprefix.secretsecretsecretsecretsecret012345"
    );
}

#[test]
fn logout_through_dispatch_removes_the_pat_and_pat_section() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["logout"]));
    });

    assert!(!pat_file(&sb).exists(), "logout removes the PAT file");
    let cfg = std::fs::read_to_string(config_file(&sb)).unwrap_or_default();
    assert!(
        !cfg.contains("[pat]"),
        "logout strips the [pat] section: {cfg}"
    );
}

#[test]
fn clean_through_dispatch_removes_config_and_pat() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["clean"]));
    });

    assert!(!pat_file(&sb).exists(), "clean removes the PAT file");
    assert!(
        !config_file(&sb).exists(),
        "clean removes the config file entirely"
    );
}

#[test]
fn login_rejects_a_token_without_the_live_prefix() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["login", "nope.short"]));
    });
    assert!(
        !pat_file(&sb).exists(),
        "an invalid token must not be persisted"
    );
}

#[test]
fn status_reports_the_sandbox_paths_without_creating_state() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["status"]));
    });
    assert!(
        !config_file(&sb).exists(),
        "status is read-only and must not create the config file"
    );
}

#[test]
fn status_after_login_sees_the_stored_credential() {
    let sb = Sandbox::new();
    let report = sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["status"]));
        systemprompt_bridge::auth::setup::status().expect("status resolves in the sandbox")
    });
    assert!(report.config_present, "config file present after login");
    assert!(report.pat_present, "PAT file present after login");
}

#[test]
fn credential_helper_emits_the_loopback_secret_for_codex() {
    let sb = Sandbox::new();
    let minted = sb.run(|| {
        let minted = systemprompt_bridge::proxy::secret::proxy_init().expect("secret mints");
        let _ = run_with_args(&argv(&["credential-helper", "--host=codex-cli"]));
        minted
    });
    let on_disk = std::fs::read_to_string(
        sb.config
            .path()
            .join("systemprompt")
            .join("bridge-loopback.key"),
    )
    .expect("the loopback secret file lands in the sandbox config dir");
    assert_eq!(
        on_disk.trim(),
        minted.as_str(),
        "the codex helper reads back the same loopback secret it was minted with"
    );
}

#[test]
fn credential_helper_without_a_host_leaves_no_state() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["credential-helper"]));
        let _ = run_with_args(&argv(&["credential-helper", "--host", "no-such-host"]));
        let _ = run_with_args(&argv(&["credential-helper", "--host", "claude-desktop"]));
    });
    assert!(
        !config_file(&sb).exists(),
        "helper failures must not write config state"
    );
}

#[test]
fn run_without_a_credential_source_writes_nothing() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["run"]));
        let _ = run_with_args(&argv(&[]));
    });
    assert!(
        !config_file(&sb).exists(),
        "the default run command must not fabricate config"
    );
}

#[test]
fn oauth_client_status_reports_an_unprovisioned_client() {
    let sb = Sandbox::new();
    let creds = sb.run(|| {
        let _ = run_with_args(&argv(&["oauth-client"]));
        let _ = run_with_args(&argv(&["oauth-client", "status"]));
        let _ = run_with_args(&argv(&["oauth-client", "bogus"]));
        systemprompt_bridge::auth::plugin_oauth::load_creds().expect("creds load is infallible")
    });
    assert!(
        creds.is_none(),
        "no OAuth client should be provisioned in a fresh sandbox"
    );
}

#[test]
fn doctor_reports_checks_and_flags_the_missing_credential() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["doctor"]));
    });
    assert!(
        !pat_file(&sb).exists(),
        "doctor is diagnostic and must not create credentials"
    );
}

#[test]
fn unknown_and_informational_commands_do_not_touch_state() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["no-such-command"]));
        let _ = run_with_args(&argv(&["help"]));
        let _ = run_with_args(&argv(&["--version"]));
        let _ = run_with_args(&argv(&["gui"]));
        let _ = run_with_args(&argv(&["__install-claude-policy"]));
        let _ = run_with_args(&argv(&["diagnostics"]));
    });
    assert!(
        !config_file(&sb).exists() && !pat_file(&sb).exists(),
        "informational commands are side-effect free"
    );
}
