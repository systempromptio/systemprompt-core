use std::process::Command;
#[cfg(unix)]
use std::time::{Duration, Instant};
use systemprompt_marketplace::managed::git_execution::{GitExecutionLimits, execute};

#[cfg(unix)]
#[test]
fn ignores_ambient_git_helpers_configuration_and_credentials() {
    let mut command = Command::new("/usr/bin/env");
    command
        .env("AWS_SECRET_ACCESS_KEY", "ambient-secret")
        .env("GIT_CONFIG_GLOBAL", "/attacker/config")
        .env("GIT_CONFIG_COUNT", "42")
        .env("GIT_CONFIG_KEY_41", "credential.helper");
    let bytes =
        execute(&mut command, None, GitExecutionLimits::default()).expect("bounded command");
    let environment = String::from_utf8(bytes).expect("utf8");
    assert!(!environment.contains("ambient-secret"));
    assert!(!environment.contains("/attacker/config"));
    assert!(!environment.contains("GIT_CONFIG_KEY_41"));
    assert!(environment.contains("GIT_CONFIG_VALUE_1=false"));
    assert!(environment.contains("GIT_ALLOW_PROTOCOL=https"));
}

#[cfg(unix)]
#[test]
fn independent_credentials_are_scoped_and_rotation_has_no_residual_state() {
    for (repository, token) in [
        ("https://one.example/a.git", "first"),
        ("https://two.example/b.git", "second"),
        ("https://one.example/a.git", "rotated"),
    ] {
        let bytes = execute(
            &mut Command::new("/usr/bin/env"),
            Some((repository, token)),
            GitExecutionLimits::default(),
        )
        .expect("command");
        let environment = String::from_utf8(bytes).expect("utf8");
        assert!(environment.contains(&format!("http.{repository}.extraHeader")));
        assert!(environment.contains(&format!("Authorization: Bearer {token}")));
        assert!(!environment.contains("Authorization: Bearer ambient"));
    }
    let bytes = execute(
        &mut Command::new("/usr/bin/env"),
        None,
        GitExecutionLimits::default(),
    )
    .expect("public request");
    assert!(
        !String::from_utf8(bytes)
            .expect("utf8")
            .contains("Authorization:")
    );
}

#[cfg(unix)]
#[test]
fn failures_redact_git_output_and_credentials() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "echo secret-output >&2; exit 1"]);
    let error = execute(
        &mut command,
        Some(("https://example.com/r.git", "private-token")),
        GitExecutionLimits::default(),
    )
    .expect_err("failed command");
    assert!(!error.to_string().contains("secret-output"));
    assert!(!error.to_string().contains("private-token"));
}

#[cfg(unix)]
#[test]
fn output_overflow_and_timeout_are_bounded() {
    let limits = GitExecutionLimits {
        deadline: Duration::from_millis(150),
        output_bytes: 32,
    };
    let mut output = Command::new("/bin/sh");
    output.args(["-c", "while :; do echo excessive-output; done"]);
    assert!(execute(&mut output, None, limits).is_err());
    let started = Instant::now();
    let mut sleeper = Command::new("/bin/sh");
    sleeper.args(["-c", "sleep 10"]);
    assert!(execute(&mut sleeper, None, limits).is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn rejects_header_injection_before_spawning() {
    let mut command = Command::new("nonexistent-executable");
    let error = execute(
        &mut command,
        Some(("https://example.com/r.git", "token\r\nInjected: yes")),
        GitExecutionLimits::default(),
    )
    .expect_err("invalid header");
    assert!(
        error
            .to_string()
            .contains("Invalid resolved Git credential")
    );
}
