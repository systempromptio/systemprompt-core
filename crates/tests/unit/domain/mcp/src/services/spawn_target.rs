use std::path::Path;

use systemprompt_config::paths::{AppPaths, BuildPaths};
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_mcp::services::process::spawner::{spawn_command_line, verify_binary};
use systemprompt_mcp::services::spawn_target::{
    HOSTED_SERVE_ARGS, HOSTED_SERVER_FLAG, SpawnProgram, SpawnTarget, hosted_serve_args,
};

use crate::harness::{external_mcp_config, internal_mcp_config};

fn paths(bin: &Path) -> AppPaths {
    let tmp = std::env::temp_dir().display().to_string();
    AppPaths::from_profile(
        &PathsConfig {
            root: None,
            system: tmp.clone(),
            services: tmp.clone(),
            bin: bin.display().to_string(),
            web_path: Some(tmp.clone()),
            storage: Some(tmp),
            geoip_database: None,
            shipped_files: None,
            data: None,
        },
        systemprompt_manifest::PathResolution::Canonicalize,
        None,
    )
    .expect("paths")
}

#[test]
fn a_server_naming_a_binary_spawns_that_binary() {
    let config = internal_mcp_config("withbin", 5401);

    assert_eq!(
        config.spawn_program().expect("spawnable"),
        SpawnProgram::Binary("withbin-bin")
    );
}

#[test]
fn an_internal_server_without_a_binary_is_hosted_by_this_binary() {
    let mut config = internal_mcp_config("hosted", 5402);
    config.binary = None;

    assert_eq!(
        config.spawn_program().expect("spawnable"),
        SpawnProgram::Hosted
    );
}

#[test]
fn a_blank_binary_is_not_spawnable() {
    let mut config = internal_mcp_config("blank", 5403);
    config.binary = Some("  ".to_owned());

    let err = config.spawn_program().expect_err("blank binary");
    assert!(err.to_string().contains("declares no binary"), "{err}");
}

#[test]
fn an_external_server_is_never_spawned() {
    let config = external_mcp_config("remote", "https://mcp.example.test/mcp");

    assert!(config.spawn_program().is_err());
}

#[test]
fn the_hosted_command_line_is_this_executable_serving_the_server() {
    let mut config = internal_mcp_config("hosted_cli", 5404);
    config.binary = None;
    let bin = tempfile::tempdir().expect("bin dir");

    let (program, args) = spawn_command_line(&paths(bin.path()), &config).expect("command line");

    assert_eq!(program, BuildPaths::resolve_self().expect("self"));
    assert_eq!(args, hosted_serve_args(&config));
    assert_eq!(
        args,
        HOSTED_SERVE_ARGS
            .iter()
            .map(|arg| (*arg).to_owned())
            .chain([HOSTED_SERVER_FLAG.to_owned(), "hosted_cli".to_owned()])
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_binary_command_line_carries_no_arguments() {
    let bin = tempfile::tempdir().expect("bin dir");
    std::fs::write(bin.path().join("withargs-bin"), b"#!/bin/sh\n").expect("stub binary");
    let config = internal_mcp_config("withargs", 5405);

    let (program, args) = spawn_command_line(&paths(bin.path()), &config).expect("command line");

    assert_eq!(program.file_name().and_then(|n| n.to_str()), Some("withargs-bin"));
    assert!(args.is_empty());
}

#[test]
fn a_hosted_server_has_no_binary_to_verify() {
    let mut config = internal_mcp_config("hosted_verify", 5406);
    config.binary = None;
    let bin = tempfile::tempdir().expect("bin dir");

    verify_binary(&paths(bin.path()), &config).expect("nothing to verify");
}
