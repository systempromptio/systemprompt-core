use systemprompt_marketplace::{ImportOptions, ImportWarning, import_anthropic_tree};
use tempfile::TempDir;

use crate::import_tree::fixture;

#[test]
fn a_forbidden_sidecar_key_names_the_key_and_where_it_comes_from() {
    let dest = TempDir::new().expect("tempdir");
    let err = import_anthropic_tree(&fixture("bad"), dest.path(), &ImportOptions::default())
        .expect_err("forbidden key must be refused");
    let message = err.to_string();

    assert!(message.contains("marketplace.version"), "{message}");
    assert!(message.contains("derived"), "{message}");
}

#[test]
fn inline_mcp_servers_warn_by_default_and_fail_under_strict() {
    let source = TempDir::new().expect("tempdir");
    copy_tree(&fixture("bad"), source.path());
    std::fs::remove_file(source.path().join(".claude-plugin/systemprompt.yaml"))
        .expect("drop the bad sidecar");

    let lenient = TempDir::new().expect("tempdir");
    let report = import_anthropic_tree(source.path(), lenient.path(), &ImportOptions::default())
        .expect("lenient import succeeds");
    assert!(report.warnings.contains(&ImportWarning::InlineMcpServers {
        plugin: "gamma".to_owned()
    }));

    let strict_dest = TempDir::new().expect("tempdir");
    let err = import_anthropic_tree(
        source.path(),
        strict_dest.path(),
        &ImportOptions {
            strict: true,
            dry_run: false,
        },
    )
    .expect_err("strict import must refuse inline mcp servers");
    assert!(err.to_string().contains("MCP servers"), "{err}");
}

#[test]
fn a_non_empty_destination_is_refused() {
    let dest = TempDir::new().expect("tempdir");
    std::fs::write(dest.path().join("leftover.txt"), b"x").expect("write");

    let err = import_anthropic_tree(
        &fixture("anthropic"),
        dest.path(),
        &ImportOptions::default(),
    )
    .expect_err("non-empty destination must be refused");
    assert!(err.to_string().contains("not empty"), "{err}");
}

#[test]
fn a_dry_run_reports_everything_and_writes_nothing() {
    let dest = TempDir::new().expect("tempdir");
    std::fs::write(dest.path().join("leftover.txt"), b"x").expect("write");

    let report = import_anthropic_tree(
        &fixture("anthropic"),
        dest.path(),
        &ImportOptions {
            strict: false,
            dry_run: true,
        },
    )
    .expect("dry run ignores a non-empty destination");

    assert_eq!(report.plugins.len(), 2);
    assert_eq!(report.skills.len(), 3);

    let entries: Vec<String> = std::fs::read_dir(dest.path())
        .expect("read dest")
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    assert_eq!(entries, vec!["leftover.txt".to_owned()]);
}

#[test]
fn a_missing_marketplace_manifest_is_a_strict_error() {
    let source = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(source.path().join("systemprompt/mcp")).expect("mkdir");

    let dest = TempDir::new().expect("tempdir");
    let err = import_anthropic_tree(
        source.path(),
        dest.path(),
        &ImportOptions {
            strict: true,
            dry_run: false,
        },
    )
    .expect_err("strict import needs a marketplace manifest");
    assert!(err.to_string().contains("marketplace.json"), "{err}");
}

fn copy_tree(src: &std::path::Path, dest: &std::path::Path) {
    for entry in std::fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&target).expect("mkdir");
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copy");
        }
    }
}
