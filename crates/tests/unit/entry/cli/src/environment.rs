//! Unit tests for the `environment` module.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_cli::environment::ExecutionEnvironment;

#[test]
fn from_env_empty_snapshot_is_all_false() {
    let overrides = EnvOverrides::from_vars(std::iter::empty::<(String, String)>());
    let env = ExecutionEnvironment::from_env(&overrides);
    assert!(!env.is_fly);
    assert!(!env.is_remote_cli);
}

#[test]
fn from_env_maps_fly_flag() {
    let overrides = EnvOverrides::from_vars([("FLY_APP_NAME", "my-app")]);
    let env = ExecutionEnvironment::from_env(&overrides);
    assert!(env.is_fly);
    assert!(!env.is_remote_cli);
}

#[test]
fn from_env_maps_remote_cli_flag() {
    let overrides = EnvOverrides::from_vars([("SYSTEMPROMPT_CLI_REMOTE", "1")]);
    let env = ExecutionEnvironment::from_env(&overrides);
    assert!(!env.is_fly);
    assert!(env.is_remote_cli);
}

#[test]
fn execution_environment_is_copy_clone_debug() {
    let overrides = EnvOverrides::from_vars([("FLY_APP_NAME", "my-app")]);
    let env = ExecutionEnvironment::from_env(&overrides);
    let copied = env;
    let cloned = env.clone();
    let _debug = format!("{:?}", env);
    assert_eq!(copied.is_fly, env.is_fly);
    assert_eq!(cloned.is_remote_cli, env.is_remote_cli);
}
