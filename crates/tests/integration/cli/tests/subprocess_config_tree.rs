//! Subprocess coverage for the `admin config` sub-trees that mutate the
//! profile and AI policy files: provider, catalog, gateway, runtime, server,
//! security, paths, governance, and secret.

use systemprompt_cli_integration_tests::full_bootstrap::{command, fixture, run, run_with_formats};

fn run_ok(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().success();
}

fn run_err(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().failure();
}

#[test]
fn provider_list_with_formats() {
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "provider", "list"]);
    run_with_formats(&["admin", "config", "provider", "list"]);
}

#[test]
fn provider_enable_disable_and_set_default() {
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "provider", "enable", "openai"]);
    run_ok(&["admin", "config", "provider", "set", "openai"]);
    run_ok(&["admin", "config", "provider", "disable", "openai"]);
    run_err(&["admin", "config", "provider", "set", "openai"]);
    run_err(&["admin", "config", "provider", "set", "nonexistent"]);
    run_err(&["admin", "config", "provider", "enable", "nonexistent"]);
    run_ok(&["admin", "config", "provider", "set", "anthropic"]);
}

#[test]
fn catalog_provider_add_list_remove() {
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "catalog", "provider", "list"]);
    run_ok(&[
        "admin",
        "config",
        "catalog",
        "provider",
        "add",
        "--name",
        "minimax",
        "--wire",
        "openai-chat",
        "--surface",
        "backend",
        "--endpoint",
        "https://api.minimax.example.com/v1/chat/completions",
        "--api-key-secret",
        "minimax",
        "--header",
        "x-extra=1",
    ]);
    run_err(&[
        "admin",
        "config",
        "catalog",
        "provider",
        "add",
        "--name",
        "badwire",
        "--wire",
        "not-a-wire",
        "--surface",
        "backend",
        "--endpoint",
        "https://api.example.com",
        "--api-key-secret",
        "x",
    ]);
    run_err(&[
        "admin",
        "config",
        "catalog",
        "provider",
        "add",
        "--name",
        "badsurface",
        "--wire",
        "anthropic",
        "--surface",
        "nope",
        "--endpoint",
        "https://api.example.com",
        "--api-key-secret",
        "x",
    ]);
    run_err(&[
        "admin",
        "config",
        "catalog",
        "provider",
        "add",
        "--name",
        "badheader",
        "--wire",
        "anthropic",
        "--surface",
        "backend",
        "--endpoint",
        "https://api.example.com",
        "--api-key-secret",
        "x",
        "--header",
        "novalue",
    ]);
    run_with_formats(&["admin", "config", "catalog", "provider", "list"]);
    run_ok(&[
        "admin", "config", "catalog", "provider", "remove", "--name", "minimax",
    ]);
    run_err(&[
        "admin", "config", "catalog", "provider", "remove", "--name", "minimax",
    ]);
}

#[test]
fn catalog_model_add_remove() {
    if fixture().is_none() {
        return;
    }
    run_ok(&[
        "admin",
        "config",
        "catalog",
        "model",
        "add",
        "--provider",
        "anthropic",
        "--id",
        "claude-haiku-4-5",
        "--alias",
        "haiku",
        "--upstream-model",
        "claude-haiku-4-5-latest",
    ]);
    run_ok(&[
        "admin",
        "config",
        "catalog",
        "model",
        "remove",
        "--provider",
        "anthropic",
        "--id",
        "claude-haiku-4-5",
    ]);
    run_err(&[
        "admin",
        "config",
        "catalog",
        "model",
        "add",
        "--provider",
        "missing",
        "--id",
        "some-model",
    ]);
}

#[test]
fn gateway_enable_routes_default_provider() {
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "gateway", "route", "list"]);
    run_ok(&["admin", "config", "gateway", "enable"]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "route",
        "add",
        "--model-pattern",
        "claude-*",
        "--provider",
        "anthropic",
        "--upstream-model",
        "claude-sonnet-4-5",
    ]);
    run_with_formats(&["admin", "config", "gateway", "route", "list"]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "default-provider",
        "set",
        "--provider",
        "anthropic",
    ]);
    run_err(&[
        "admin",
        "config",
        "gateway",
        "default-provider",
        "set",
        "--provider",
        "missing",
    ]);
    run_ok(&["admin", "config", "gateway", "default-provider", "clear"]);
    run_ok(&[
        "admin",
        "config",
        "gateway",
        "route",
        "remove",
        "--model-pattern",
        "claude-*",
    ]);
    run_err(&[
        "admin",
        "config",
        "gateway",
        "route",
        "remove",
        "--model-pattern",
        "claude-*",
    ]);
    run_ok(&["admin", "config", "gateway", "disable"]);
}

#[test]
fn runtime_show_and_set() {
    if fixture().is_none() {
        return;
    }
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
    if fixture().is_none() {
        return;
    }
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
        "--use-https",
        "false",
        "--api-server-url",
        "http://0.0.0.0",
        "--api-internal-url",
        "http://0.0.0.0",
        "--api-external-url",
        "http://0.0.0.0:8090",
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
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "security", "show"]);
    run_with_formats(&["admin", "config", "security", "show"]);
    run_ok(&[
        "admin",
        "config",
        "security",
        "set",
        "--jwt-issuer",
        "test2",
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
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "paths", "show"]);
    run_with_formats(&["admin", "config", "paths", "show"]);
    run_with_formats(&["admin", "config", "paths", "validate"]);
}

#[test]
fn governance_show_and_set() {
    if fixture().is_none() {
        return;
    }
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
    if fixture().is_none() {
        return;
    }
    run(&["admin", "config", "secret", "set", "anthropic", "sk-test-1"]);
    run(&["admin", "config", "secret", "set", "customsecret", "value1"]);
}

#[test]
fn config_show_list_validate_with_formats() {
    if fixture().is_none() {
        return;
    }
    run_ok(&["admin", "config", "show"]);
    run_with_formats(&["admin", "config", "list"]);
    run_with_formats(&["admin", "config", "validate"]);
}
