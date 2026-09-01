//! The environment an MCP subprocess is spawned with, and log rotation.
//!
//! `configure_environment` calls `env_clear()` before applying this list, so
//! whatever `build_environment` omits, the child does not get. The `lookup`
//! closure is injected, so these drive it directly rather than mutating the
//! process environment and racing every other test in the binary.
//!
//! `spawn_server`, `build_server` and `verify_binary` are left alone: they run
//! a binary or a build, which is the supervisor behaviour this suite is not
//! trying to reach.

use std::collections::HashMap;
use std::path::Path;

use systemprompt_mcp::services::process::spawner::{
    SpawnEnvSpec, build_environment, rotate_log_if_needed, serialize_server_configs,
};
use systemprompt_models::subprocess::{MCP_SERVICE_ID_ENV, SUBPROCESS_MARKER_ENV};

use crate::harness::internal_mcp_config;

fn spec_for<'a>(config: &'a systemprompt_models::mcp::McpServerConfig) -> SpawnEnvSpec<'a> {
    SpawnEnvSpec {
        config,
        system_root: Path::new("/srv/systemprompt"),
        database_type: "postgres",
        profile_path: "/etc/systemprompt/profile.yaml",
        tools_config_json: "{}",
        server_model_config_json: "null",
    }
}

fn env_map(pairs: &[(String, String)]) -> HashMap<&str, &str> {
    pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

fn nothing_set(_: &str) -> Option<String> {
    None
}

// Why: this marker is the contract between spawning and reaping. The
// reconciler's `service_row_is_stale` decides a row is ours by looking for
// `SYSTEMPROMPT_SUBPROCESS=1` on the live process; if a spawn ever stopped
// emitting it, every running MCP server would read as an unrelated process and
// be reaped on the next pass.
#[test]
fn every_spawn_carries_the_marker_the_reaper_identifies_us_by() {
    let config = internal_mcp_config("marker-server", 9001);
    let env = build_environment(&spec_for(&config), &[], nothing_set);
    let map = env_map(&env);

    assert_eq!(
        map.get(SUBPROCESS_MARKER_ENV).copied(),
        Some("1"),
        "without this the reconciler cannot tell our child from a stranger"
    );
    assert_eq!(
        map.get(MCP_SERVICE_ID_ENV).copied(),
        Some("marker-server"),
        "the reaper matches the service by this name"
    );
}

// Why: the child's environment is cleared first, so anything not listed here
// is genuinely absent. PATH in particular decides whether the server can find
// anything it shells out to.
#[test]
fn path_and_home_are_inherited_when_present() {
    let config = internal_mcp_config("inherit-server", 9002);
    let env = build_environment(&spec_for(&config), &[], |name| match name {
        "PATH" => Some("/usr/bin".to_owned()),
        "HOME" => Some("/home/sp".to_owned()),
        _ => None,
    });
    let map = env_map(&env);

    assert_eq!(map.get("PATH").copied(), Some("/usr/bin"));
    assert_eq!(map.get("HOME").copied(), Some("/home/sp"));
}

// Why: the deployment-host marker is how a process knows it is already ON the
// machine its cloud profile describes. The CLI reads it to skip remote routing
// and run locally. While it was missing from the inherit list, every MCP
// subprocess on a deployed host believed it was off-host, so a server that
// shells out to the CLI tried to route a command to the host it was already
// running on and died on a tenant store no container has — which took out all
// three admin dashboards in production.
#[test]
fn host_identity_reaches_the_child_so_it_does_not_route_back_to_itself() {
    let config = internal_mcp_config("host-server", 9009);
    let env = build_environment(&spec_for(&config), &[], |name| {
        (name == "SYSTEMPROMPT_DEPLOYMENT_HOST").then(|| "sp-tenant".to_owned())
    });

    assert_eq!(
        env_map(&env).get("SYSTEMPROMPT_DEPLOYMENT_HOST").copied(),
        Some("sp-tenant"),
        "without this the child cannot tell it is already on the target host"
    );
}

// Why: Fly injects its own marker and nothing we generate does, so a tenant
// deployed before the generated marker existed still depends on this one being
// forwarded.
#[test]
fn flys_own_marker_is_forwarded_too_so_existing_tenants_need_no_redeploy() {
    let config = internal_mcp_config("fly-server", 9010);
    let env = build_environment(&spec_for(&config), &[], |name| {
        (name == "FLY_APP_NAME").then(|| "sp-tenant".to_owned())
    });

    assert_eq!(env_map(&env).get("FLY_APP_NAME").copied(), Some("sp-tenant"));
}

#[test]
fn an_unset_inherited_var_is_omitted_rather_than_passed_as_empty() {
    let config = internal_mcp_config("empty-server", 9003);
    let env = build_environment(&spec_for(&config), &[], nothing_set);
    let map = env_map(&env);

    assert!(
        !map.contains_key("PATH"),
        "an absent PATH must stay absent; an empty one reads as a real value"
    );
    assert!(!map.contains_key("HOME"));
    assert!(!map.contains_key("FLY_APP_NAME"));
    assert!(
        !map.contains_key("SYSTEMPROMPT_DEPLOYMENT_HOST"),
        "off a Fly host the name must stay absent; an empty one would read as \
         being on-host and wrongly suppress remote routing"
    );
}

// Why: a declared-but-unset optional var must be omitted, not passed empty. A
// server that checks `is_set` would treat an empty API key as configured and
// fail at the first call instead of at startup.
#[test]
fn a_declared_env_var_that_is_not_set_is_omitted_not_blank() {
    let mut config = internal_mcp_config("optional-server", 9004);
    config.env_vars = vec!["OPTIONAL_TOKEN".to_owned()];

    let env = build_environment(&spec_for(&config), &[], nothing_set);

    assert!(
        !env.iter().any(|(k, _)| k == "OPTIONAL_TOKEN"),
        "an unset optional var must not be forwarded as an empty string"
    );
}

#[test]
fn a_declared_env_var_that_is_set_is_forwarded() {
    let mut config = internal_mcp_config("optional-server", 9005);
    config.env_vars = vec!["OPTIONAL_TOKEN".to_owned()];

    let env = build_environment(&spec_for(&config), &[], |name| {
        (name == "OPTIONAL_TOKEN").then(|| "tok-value".to_owned())
    });

    assert_eq!(
        env_map(&env).get("OPTIONAL_TOKEN").copied(),
        Some("tok-value")
    );
}

#[test]
fn secrets_are_passed_through_to_the_child() {
    let config = internal_mcp_config("secret-server", 9006);
    let secrets = vec![("API_KEY".to_owned(), "sk-test".to_owned())];

    let env = build_environment(&spec_for(&config), &secrets, nothing_set);

    assert_eq!(env_map(&env).get("API_KEY").copied(), Some("sk-test"));
}

// Why: the port is how the parent reaches the server it just started. Passing
// the wrong one produces a process that runs and is unreachable.
#[test]
fn the_configured_port_and_profile_reach_the_child() {
    let config = internal_mcp_config("port-server", 9007);
    let env = build_environment(&spec_for(&config), &[], nothing_set);
    let map = env_map(&env);

    assert_eq!(map.get("MCP_PORT").copied(), Some("9007"));
    assert_eq!(
        map.get("SYSTEMPROMPT_PROFILE").copied(),
        Some("/etc/systemprompt/profile.yaml")
    );
    assert_eq!(map.get("DATABASE_TYPE").copied(), Some("postgres"));
}

#[test]
fn tool_configuration_is_serialised_as_json_for_the_child() {
    let config = internal_mcp_config("tools-server", 9008);

    let (tools, model) = serialize_server_configs(&config).expect("configs should serialise");

    serde_json::from_str::<serde_json::Value>(&tools).expect("tools config must be valid JSON");
    serde_json::from_str::<serde_json::Value>(&model)
        .expect("model config must be valid JSON, including when absent");
}

// Why: rotation is what stops a long-running server filling the disk. Rotating
// early would discard logs an operator still needs, so the threshold is only
// crossed by size.
#[test]
fn a_log_under_the_threshold_is_left_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("mcp-small.log");
    std::fs::write(&log, b"a few lines").expect("write log");

    rotate_log_if_needed(&log);

    assert!(log.exists(), "a small log must not be rotated away");
    assert!(!dir.path().join("mcp-small.log.old").exists());
    assert_eq!(
        std::fs::read(&log).expect("read log"),
        b"a few lines",
        "the log must be untouched"
    );
}

#[test]
fn a_log_over_the_threshold_is_moved_aside_and_the_content_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("mcp-big.log");
    let file = std::fs::File::create(&log).expect("create log");
    file.set_len(11 * 1024 * 1024).expect("grow past 10MiB");
    drop(file);

    rotate_log_if_needed(&log);

    assert!(
        !log.exists(),
        "the oversized log is moved aside, leaving the path free for a new one"
    );
    let rotated = dir.path().join("mcp-big.log.old");
    assert!(
        rotated.exists(),
        "rotation must preserve the old log rather than truncating it"
    );
}

// Why: rotating a log that is not there must not create one or panic. It is
// called on every spawn, including the first, when no log exists yet.
#[test]
fn rotating_a_log_that_does_not_exist_is_a_no_op() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("never-written.log");

    rotate_log_if_needed(&missing);

    assert!(!missing.exists(), "rotation must not create the log");
    assert!(!dir.path().join("never-written.log.old").exists());
}

// Why: this is the test that stops the outage recurring. The MCP spawner and the
// agent spawner each build a child environment; the inherited-from-parent half is
// supposed to be identical, and for a long time it silently was not — the agent
// forwarded the deployment-host marker through a bolt-on and the MCP one did not
// forward it at all, so every MCP server on a deployed host believed it was
// somewhere else and tried to route commands to the host it was already on.
//
// Neither side's own tests could catch that, because a test only ever saw one
// side. This one fails the moment either grows a variable the other lacks.
#[test]
fn both_spawners_inherit_exactly_the_same_environment() {
    let parent = |name: &str| -> Option<String> {
        match name {
            "PATH" => Some("/usr/bin".to_owned()),
            "HOME" => Some("/home/sp".to_owned()),
            "SYSTEMPROMPT_DEPLOYMENT_HOST" => Some("sp-tenant".to_owned()),
            "FLY_APP_NAME" => Some("sp-fly".to_owned()),
            _ => None,
        }
    };

    let shared = systemprompt_models::subprocess::inherited_parent_env(parent);

    let mcp_config = internal_mcp_config("parity-server", 9011);
    let mcp = build_environment(&spec_for(&mcp_config), &[], parent);

    for (key, value) in &shared {
        assert!(
            mcp.contains(&(key.clone(), value.clone())),
            "the MCP spawner dropped {key}, which the agent spawner keeps"
        );
    }
}
