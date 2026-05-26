//! Unit tests for the `environment` module.
//!
//! Note: `ExecutionEnvironment::detect` reads process-global env vars, and
//! tests run in parallel within a single process. We don't mutate env vars
//! here — we just assert the detect result is structurally consistent and
//! the struct's derives behave.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::environment::ExecutionEnvironment;

#[test]
fn detect_returns_consistent_struct() {
    let env = ExecutionEnvironment::detect();
    let env2 = ExecutionEnvironment::detect();
    assert_eq!(env.is_fly, env2.is_fly);
    assert_eq!(env.is_remote_cli, env2.is_remote_cli);
}

#[test]
fn detect_matches_env_vars() {
    let env = ExecutionEnvironment::detect();
    assert_eq!(env.is_fly, std::env::var("FLY_APP_NAME").is_ok());
    assert_eq!(
        env.is_remote_cli,
        std::env::var("SYSTEMPROMPT_CLI_REMOTE").is_ok()
    );
}

#[test]
fn execution_environment_is_copy_clone_debug() {
    let env = ExecutionEnvironment::detect();
    let copied = env;
    let cloned = env.clone();
    let _debug = format!("{:?}", env);
    assert_eq!(copied.is_fly, env.is_fly);
    assert_eq!(cloned.is_remote_cli, env.is_remote_cli);
}
