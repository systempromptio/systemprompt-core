//! Subprocess coverage for the `web` tree and the DB-backed `core content`,
//! `core contexts`, and `core artifacts` trees.
//!
//! Content-type mutations use a dedicated `covcli_ct` name and run the whole
//! create/show/edit/delete cycle inside one test so ordering is
//! self-contained. Tests accept success or failure exit codes.

use systemprompt_cli_integration_tests::full_bootstrap::{run, run_with_formats};

#[test]
fn content_types_lifecycle() {
    run(&[
        "web",
        "content-types",
        "create",
        "--name",
        "covcli_ct",
        "--path",
        "content/covcli",
        "--source-id",
        "covcli",
        "--description",
        "coverage fixture content type",
        "--url-pattern",
        "/covcli/{slug}",
        "--priority",
        "0.4",
    ]);
    run_with_formats(&["web", "content-types", "show", "covcli_ct"]);
    run(&["web", "content-types", "edit", "covcli_ct", "--disable"]);
    run(&["web", "content-types", "edit", "covcli_ct", "--enable"]);
    run(&[
        "web",
        "content-types",
        "edit",
        "covcli_ct",
        "--priority",
        "0.9",
        "--changefreq",
        "daily",
    ]);
    run(&["web", "content-types", "delete", "covcli_ct", "--yes"]);
}

#[test]
fn content_types_edit_missing() {
    run(&["web", "content-types", "edit", "no_such_ct", "--disable"]);
}

#[test]
fn content_types_create_invalid_priority() {
    run(&[
        "web",
        "content-types",
        "create",
        "--name",
        "covcli_bad_ct",
        "--path",
        "content/bad",
        "--source-id",
        "bad",
        "--priority",
        "9.9",
    ]);
}

#[test]
fn templates_show_and_edit_missing() {
    run_with_formats(&["web", "templates", "list"]);
    run(&["web", "templates", "show", "no_such_template"]);
    run(&[
        "web",
        "templates",
        "edit",
        "no_such_template",
        "--content-types",
        "blog",
    ]);
}

#[test]
fn assets_and_sitemap() {
    run_with_formats(&["web", "assets", "list"]);
    run(&["web", "assets", "show", "no_such_asset"]);
    run(&["web", "sitemap", "generate"]);
    run(&["web", "validate"]);
}

#[test]
fn content_query_trees() {
    run_with_formats(&["core", "content", "list"]);
    run(&["core", "content", "list", "--limit", "5"]);
    run(&["core", "content", "search", "coverage"]);
    run(&["core", "content", "show", "no-such-content-id"]);
    run(&["core", "content", "popular"]);
    run(&["core", "content", "status", "covcli"]);
    run(&[
        "core",
        "content",
        "delete-source",
        "no_such_source",
        "--yes",
    ]);
}

#[test]
fn contexts_lifecycle() {
    run_with_formats(&["core", "contexts", "list"]);
    run(&["core", "contexts", "create", "covcli_context"]);
    run(&["core", "contexts", "show", "covcli_context"]);
    run(&[
        "core",
        "contexts",
        "edit",
        "covcli_context",
        "covcli_context_renamed",
    ]);
    run(&[
        "core",
        "contexts",
        "delete",
        "covcli_context_renamed",
        "--yes",
    ]);
    run(&["core", "contexts", "show", "no_such_context"]);
}

#[test]
fn artifacts_queries() {
    run_with_formats(&["core", "artifacts", "list"]);
    run(&["core", "artifacts", "list", "--limit", "5"]);
    run(&["core", "artifacts", "show", "no-such-artifact"]);
    run(&["core", "artifacts", "show", "no-such-artifact", "--full"]);
}

#[test]
fn files_queries() {
    run_with_formats(&["core", "files", "list"]);
    run(&["core", "files", "show", "no-such-file"]);
    run(&["core", "files", "search", "zzz"]);
    run(&["core", "files", "stats"]);
    run(&["core", "files", "config"]);
    run(&["core", "files", "validate", "/nonexistent/file.png"]);
    run(&["core", "files", "delete", "no-such-file", "--yes"]);
}
