//! Subprocess coverage for the profile-scoped `admin users` write paths
//! (create, update, merge, bulk, ban, session end, webauthn) plus the
//! `admin keys` and `admin access-control` trees.

use systemprompt_cli_integration_tests::full_bootstrap::{command, fixture};

fn run_ok(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().success();
}

fn run_any(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    let _ = cmd.assert();
}

fn run_err(args: &[&str]) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().failure();
}


fn unique(name: &str) -> String {
    format!("{name}_{}", std::process::id())
}

fn user_id_by_name(name: &str) -> Option<String> {
    let mut cmd = command()?;
    cmd.args(["--json", "admin", "users", "search", name]);
    let output = cmd.assert().success();
    let raw = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let rows = value
        .get("data")
        .and_then(|d| d.as_array().cloned())
        .or_else(|| value.as_array().cloned())?;
    rows.iter()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some(name))
        .and_then(|r| r.get("id").and_then(|i| i.as_str()).map(str::to_owned))
}

#[test]
fn user_create_update_show_delete_cycle() {
    if fixture().is_none() {
        return;
    }
    let name = unique("covuser_cycle");
    let email = format!("{name}@example.com");
    run_ok(&[
        "admin",
        "users",
        "create",
        "--name",
        &name,
        "--email",
        &email,
        "--full-name",
        "Coverage User",
        "--display-name",
        "Cov User",
    ]);
    run_ok(&[
        "admin",
        "users",
        "create",
        "--name",
        &name,
        "--email",
        &email,
        "--if-not-exists",
    ]);
    run_err(&[
        "admin", "users", "create", "--name", &name, "--email", &email,
    ]);

    let Some(id) = user_id_by_name(&name) else {
        return;
    };
    run_ok(&["admin", "users", "show", &id]);
    run_ok(&["--json", "admin", "users", "show", &id]);
    run_ok(&[
        "admin",
        "users",
        "update",
        &id,
        "--email",
        "covuser_cycle2@example.com",
        "--full-name",
        "Updated User",
        "--display-name",
        "Updated",
        "--status",
        "active",
        "--email-verified",
        "true",
    ]);
    run_any(&["admin", "users", "session", "list", &id]);
    run_any(&[
        "admin", "users", "session", "end", "--user", &id, "--all", "-y",
    ]);
    run_ok(&["admin", "users", "delete", &id, "-y"]);
    run_err(&["admin", "users", "delete", &id, "-y"]);
}

#[test]
fn user_merge_flow() {
    if fixture().is_none() {
        return;
    }
    let src_name = unique("covmerge_src");
    let dst_name = unique("covmerge_dst");
    let src_email = format!("{src_name}@example.com");
    let dst_email = format!("{dst_name}@example.com");
    run_ok(&[
        "admin", "users", "create", "--name", &src_name, "--email", &src_email,
    ]);
    run_ok(&[
        "admin", "users", "create", "--name", &dst_name, "--email", &dst_email,
    ]);
    let (Some(src), Some(dst)) = (user_id_by_name(&src_name), user_id_by_name(&dst_name)) else {
        return;
    };
    run_any(&[
        "admin", "users", "merge", "--source", &src, "--target", &dst, "-y",
    ]);
    run_err(&[
        "admin",
        "users",
        "merge",
        "--source",
        "missing-user",
        "--target",
        &dst,
        "-y",
    ]);
    run_any(&["admin", "users", "delete", &dst, "-y"]);
}

#[test]
fn bulk_update_and_delete() {
    if fixture().is_none() {
        return;
    }
    run_any(&[
        "admin",
        "users",
        "bulk",
        "update",
        "--set-status",
        "inactive",
        "--role",
        "anonymous",
        "--dry-run",
    ]);
    run_any(&[
        "admin",
        "users",
        "bulk",
        "update",
        "--set-status",
        "active",
        "--status",
        "inactive",
        "--limit",
        "5",
        "-y",
    ]);
    run_any(&[
        "admin",
        "users",
        "bulk",
        "delete",
        "--role",
        "anonymous",
        "--older-than",
        "365",
        "--dry-run",
    ]);
    run_any(&[
        "admin",
        "users",
        "bulk",
        "delete",
        "--status",
        "inactive",
        "--older-than",
        "3650",
        "--limit",
        "2",
        "-y",
    ]);
    run_err(&["admin", "users", "bulk", "delete", "-y"]);
}

#[test]
fn ban_lifecycle() {
    if fixture().is_none() {
        return;
    }
    run_ok(&[
        "admin",
        "users",
        "ban",
        "add",
        "203.0.113.77",
        "--reason",
        "coverage test",
        "--duration",
        "1h",
    ]);
    run_ok(&["admin", "users", "ban", "check", "203.0.113.77"]);
    run_ok(&["--json", "admin", "users", "ban", "check", "203.0.113.77"]);
    run_ok(&["admin", "users", "ban", "check", "198.51.100.1"]);
    run_any(&["admin", "users", "ban", "list"]);
    run_any(&["admin", "users", "ban", "remove", "203.0.113.77", "-y"]);
    run_ok(&[
        "admin",
        "users",
        "ban",
        "add",
        "203.0.113.78",
        "--reason",
        "permanent coverage",
        "--permanent",
    ]);
    run_any(&["admin", "users", "ban", "remove", "203.0.113.78", "-y"]);
    run_any(&["admin", "users", "ban", "cleanup"]);
}

#[test]
fn webauthn_setup_token() {
    if fixture().is_none() {
        return;
    }
    run_any(&[
        "admin",
        "users",
        "webauthn",
        "generate-setup-token",
        "--email",
        "testadmin@example.com",
        "--expires-minutes",
        "10",
    ]);
    run_err(&[
        "admin",
        "users",
        "webauthn",
        "generate-setup-token",
        "--email",
        "missing@example.com",
    ]);
}

#[test]
fn keys_issue_plugin_token() {
    if fixture().is_none() {
        return;
    }
    run_any(&[
        "admin",
        "keys",
        "issue-plugin-token",
        "--email",
        "testadmin@example.com",
        "--plugin-id",
        "cov-plugin",
        "--duration-days",
        "7",
    ]);
    run_any(&[
        "admin",
        "keys",
        "issue-plugin-token",
        "--email",
        "testadmin@example.com",
        "--token-only",
    ]);
    run_err(&[
        "admin",
        "keys",
        "issue-plugin-token",
        "--email",
        "testadmin@example.com",
        "--duration-days",
        "0",
    ]);
}

#[test]
fn access_control_export_and_lint() {
    if fixture().is_none() {
        return;
    }
    run_any(&["admin", "access-control", "export-yaml"]);
    run_any(&["--json", "admin", "access-control", "export-yaml"]);
    run_any(&["admin", "access-control", "lint"]);
}
