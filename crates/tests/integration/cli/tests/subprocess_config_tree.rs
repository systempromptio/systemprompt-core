//! Subprocess coverage for the `admin config` sub-trees that mutate the
//! profile and AI policy files: provider, catalog, gateway, runtime, server,
//! security, paths, governance, and secret.

use systemprompt_cli_integration_tests::full_bootstrap::{
    cli_command, full_fixture, run, run_with_formats,
};

fn run_ok(args: &[&str]) {
    let mut cmd = cli_command();
    cmd.args(args);
    cmd.assert().success();
}

fn run_err(args: &[&str]) {
    let mut cmd = cli_command();
    cmd.args(args);
    cmd.assert().failure();
}

#[test]
fn provider_list_with_formats() {
    full_fixture();
    run_ok(&["admin", "config", "provider", "list"]);
    run_with_formats(&["admin", "config", "provider", "list"]);
}

#[test]
fn provider_enable_disable() {
    full_fixture();
    run_ok(&["admin", "config", "provider", "disable", "--id", "openai"]);
    run_ok(&["admin", "config", "provider", "enable", "--id", "openai"]);
    run_err(&[
        "admin",
        "config",
        "provider",
        "enable",
        "--id",
        "nonexistent",
    ]);
}

#[test]
fn catalog_credential_provider_endpoint_add_list_remove() {
    full_fixture();
    run_ok(&["admin", "config", "credential", "list"]);
    run_ok(&[
        "admin",
        "config",
        "credential",
        "add",
        "--id",
        "minimax",
        "--kind",
        "platform",
        "--secret",
        "minimax",
    ]);
    run_err(&[
        "admin",
        "config",
        "credential",
        "add",
        "--id",
        "bad",
        "--kind",
        "personal",
        "--secret",
        "x",
    ]);
    run_err(&[
        "admin",
        "config",
        "credential",
        "add",
        "--id",
        "bad",
        "--kind",
        "nope",
    ]);
    run_ok(&[
        "admin",
        "config",
        "provider",
        "add",
        "--id",
        "minimax",
        "--credential",
        "minimax",
        "--header",
        "x-extra=1",
        "--endpoint",
        "messages",
        "--wire",
        "anthropic",
        "--url",
        "https://api.minimax.example.com/anthropic/v1",
    ]);
    run_err(&[
        "admin",
        "config",
        "provider",
        "add",
        "--id",
        "badheader",
        "--credential",
        "minimax",
        "--header",
        "novalue",
    ]);
    run_ok(&[
        "admin",
        "config",
        "endpoint",
        "add",
        "--provider",
        "minimax",
        "--id",
        "chat",
        "--wire",
        "openai-chat",
        "--url",
        "https://api.minimax.example.com/v1",
    ]);
    run_err(&[
        "admin",
        "config",
        "endpoint",
        "add",
        "--provider",
        "minimax",
        "--id",
        "bad",
        "--wire",
        "not-a-wire",
        "--url",
        "https://api.example.com",
    ]);
    run_with_formats(&["admin", "config", "provider", "list"]);
    run_ok(&["admin", "config", "provider", "remove", "--id", "minimax"]);
    run_err(&["admin", "config", "provider", "remove", "--id", "minimax"]);
    run_ok(&["admin", "config", "credential", "remove", "--id", "minimax"]);
}

#[test]
fn catalog_model_and_deployment_add_remove() {
    full_fixture();
    run_ok(&[
        "admin",
        "config",
        "model",
        "add",
        "--id",
        "claude-haiku-4-5",
        "--deployment",
        "anthropic/api",
        "--upstream-model",
        "claude-haiku-4-5-latest",
        "--clients",
        "claude",
    ]);
    run_ok(&[
        "admin",
        "config",
        "deployment",
        "set",
        "--model",
        "claude-haiku-4-5",
        "--endpoint",
        "anthropic/api",
        "--endpoint",
        "openai/api",
    ]);
    run_with_formats(&["admin", "config", "deployment", "list"]);
    run_ok(&[
        "admin",
        "config",
        "model",
        "remove",
        "--id",
        "claude-haiku-4-5",
    ]);
    run_err(&[
        "admin",
        "config",
        "model",
        "add",
        "--id",
        "some-model",
        "--deployment",
        "missing/api",
    ]);
    run_err(&["admin", "config", "model", "add", "--id", "no-deployment"]);
}

#[test]
fn gateway_enable_routes_default_model() {
    full_fixture();
    run_ok(&["admin", "config", "gateway", "route", "list"]);
    run_ok(&["admin", "config", "gateway", "enable"]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "route",
        "add",
        "--id",
        "claude-pin",
        "--models",
        "claude-*",
        "--deployment",
        "anthropic/api",
    ]);
    run_with_formats(&["admin", "config", "gateway", "route", "list"]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "default-model",
        "set",
        "--model",
        "claude-sonnet-4-5",
    ]);
    run_err(&[
        "admin",
        "config",
        "gateway",
        "default-model",
        "set",
        "--model",
        "missing",
    ]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "route",
        "remove",
        "--id",
        "claude-pin",
    ]);
    run_err(&[
        "admin",
        "config",
        "gateway",
        "route",
        "remove",
        "--id",
        "claude-pin",
    ]);
    run_ok(&["admin", "config", "gateway", "disable"]);
}

#[test]
fn runtime_show_and_set() {
    full_fixture();
    run_ok(&["admin", "config", "runtime", "show"]);
    run_with_formats(&["admin", "config", "runtime", "show"]);
    run_ok(&[
        "admin",
        "config",
        "runtime",
        "set",
        "--environment",
        "test",
        "--log-level",
        "normal",
        "--output-format",
        "json",
        "--no-color",
        "true",
    ]);
    run_err(&[
        "admin",
        "config",
        "runtime",
        "set",
        "--environment",
        "not-an-env",
    ]);
    run_err(&["admin", "config", "runtime", "set", "--log-level", "loud"]);
    run(&["admin", "config", "runtime", "set"]);
}

#[test]
fn server_show_set_and_cors() {
    full_fixture();
    run_ok(&["admin", "config", "server", "show"]);
    run_with_formats(&["admin", "config", "server", "show"]);
    run_ok(&[
        "admin",
        "config",
        "server",
        "set",
        "--host",
        "0.0.0.0",
        "--port",
        "8090",
        "--external-url",
        "http://localhost:8090",
    ]);
    run(&["admin", "config", "server", "set"]);
    run_ok(&["admin", "config", "server", "cors", "list"]);
    run_ok(&[
        "admin",
        "config",
        "server",
        "cors",
        "add",
        "https://example.com",
    ]);
    run_with_formats(&["admin", "config", "server", "cors", "list"]);
    run_ok(&[
        "admin",
        "config",
        "server",
        "cors",
        "remove",
        "https://example.com",
    ]);
    run(&[
        "admin",
        "config",
        "server",
        "cors",
        "remove",
        "https://never-added.example.com",
    ]);
}

#[test]
fn security_show_set_and_trusted_issuers() {
    full_fixture();
    run_ok(&["admin", "config", "security", "show"]);
    run_with_formats(&["admin", "config", "security", "show"]);
    run_ok(&[
        "admin",
        "config",
        "security",
        "set",
        "--access-expiry",
        "7200",
        "--refresh-expiry",
        "172800",
    ]);
    run(&["admin", "config", "security", "set"]);
    run_ok(&[
        "admin",
        "config",
        "security",
        "trusted-issuer",
        "add",
        "--issuer",
        "https://issuer.example.com",
        "--jwks-uri",
        "https://issuer.example.com/.well-known/jwks.json",
        "--audience",
        "api",
    ]);
    run_ok(&[
        "admin",
        "config",
        "security",
        "trusted-issuer",
        "remove",
        "--issuer",
        "https://issuer.example.com",
    ]);
    run_err(&[
        "admin",
        "config",
        "security",
        "trusted-issuer",
        "remove",
        "--issuer",
        "https://unknown.example.com",
    ]);
}

#[test]
fn paths_show_and_validate() {
    full_fixture();
    run_ok(&["admin", "config", "paths", "show"]);
    run_with_formats(&["admin", "config", "paths", "show"]);
    run_with_formats(&["admin", "config", "paths", "validate"]);
}

#[test]
fn governance_show_and_set() {
    full_fixture();
    run_ok(&["admin", "config", "governance", "show"]);
    run_with_formats(&["admin", "config", "governance", "show"]);
    run(&[
        "admin",
        "config",
        "governance",
        "set",
        "--mode",
        "webhook",
        "--url",
        "http://127.0.0.1:9/hook",
        "--timeout-ms",
        "250",
    ]);
    run(&["admin", "config", "governance", "set", "--mode", "off"]);
}

#[test]
fn secret_set_provider_and_custom() {
    full_fixture();
    run(&["admin", "config", "secret", "set", "anthropic", "sk-test-1"]);
    run(&["admin", "config", "secret", "set", "customsecret", "value1"]);
}

#[test]
fn config_show_list_validate_with_formats() {
    full_fixture();
    run_ok(&["admin", "config", "show"]);
    run_with_formats(&["admin", "config", "list"]);
    run_with_formats(&["admin", "config", "validate"]);
}
