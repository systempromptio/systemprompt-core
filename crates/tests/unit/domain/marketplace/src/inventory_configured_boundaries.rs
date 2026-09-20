use super::*;
use std::collections::BTreeMap;

fn entries_by_key(
    entries: Vec<systemprompt_marketplace::inventory::ConfiguredInventoryEntry>,
) -> BTreeMap<(String, String), systemprompt_marketplace::inventory::ConfiguredInventoryEntry> {
    entries
        .into_iter()
        .map(|entry| ((entry.kind.clone(), entry.resource_key.clone()), entry))
        .collect()
}

#[test]
fn configured_services_preserve_enabled_and_disabled_availability() {
    let root = tempfile::tempdir().expect("services root");
    let services: ServicesConfig = serde_yaml::from_str(
        r#"
agents:
  active:
    name: active
    port: 4100
    endpoint: http://localhost:4100/active
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Active
      description: active fixture
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {}
  stopped:
    name: stopped
    port: 4101
    endpoint: http://localhost:4101/stopped
    enabled: false
    card:
      protocolVersion: "0.2.3"
      displayName: Stopped
      description: stopped fixture
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {}
mcp_servers:
  active:
    binary: fixture
    enabled: true
    display_in_web: false
    oauth: {required: false, scopes: [], audience: mcp, client_id: null}
  stopped:
    binary: fixture
    enabled: false
    display_in_web: false
    oauth: {required: false, scopes: [], audience: mcp, client_id: null}
"#,
    )
    .expect("valid services configuration");

    let entries = entries_by_key(
        scan_configured_inventory(root.path(), &services).expect("configured inventory"),
    );
    for (kind, root_prefix) in [("agent", "configured/agents"), ("mcp", "configured/mcp")] {
        let active = &entries[&(kind.to_owned(), "active".to_owned())];
        assert_eq!(active.availability, InventoryAvailability::Available);
        assert_eq!(active.relative_root, format!("{root_prefix}/active"));
        assert_eq!(active.diagnostic, None);

        let stopped = &entries[&(kind.to_owned(), "stopped".to_owned())];
        assert_eq!(stopped.availability, InventoryAvailability::Unavailable);
        assert_eq!(stopped.relative_root, format!("{root_prefix}/stopped"));
        assert_eq!(
            stopped.diagnostic.as_deref(),
            Some(if kind == "agent" {
                "Configured agent is disabled"
            } else {
                "Configured MCP server is disabled"
            })
        );
    }
}

#[test]
fn catalog_identity_and_disabled_state_are_retained_as_unavailable() {
    let root = tempfile::tempdir().expect("services root");
    std::fs::create_dir_all(root.path().join("plugins/expected")).expect("plugin directory");
    std::fs::write(
        root.path().join("plugins/expected/config.yaml"),
        "id: different\nenabled: true\n",
    )
    .expect("identity mismatch");
    std::fs::create_dir_all(root.path().join("rules/blocked")).expect("rule directory");
    std::fs::write(
        root.path().join("rules/blocked/config.yaml"),
        "id: blocked\nenabled: false\n",
    )
    .expect("disabled rule");

    let entries = entries_by_key(
        scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("inventory"),
    );
    let mismatched = &entries[&("plugin".to_owned(), "expected".to_owned())];
    assert_eq!(mismatched.availability, InventoryAvailability::Unavailable);
    assert_eq!(
        mismatched.diagnostic.as_deref(),
        Some("Invalid managed resource: Configured identity conflicts with its path")
    );
    let disabled = &entries[&("rule".to_owned(), "blocked".to_owned())];
    assert_eq!(disabled.availability, InventoryAvailability::Unavailable);
    assert_eq!(
        disabled.diagnostic.as_deref(),
        Some("Invalid managed resource: Configured entry is disabled")
    );
}

#[test]
fn missing_skill_instruction_is_unavailable_but_other_entries_continue() {
    let root = tempfile::tempdir().expect("services root");
    std::fs::create_dir_all(root.path().join("skills/missing")).expect("skill directory");
    std::fs::write(
        root.path().join("skills/missing/config.yaml"),
        "id: missing\nenabled: true\nfile: GUIDE.md\n",
    )
    .expect("skill config");
    std::fs::create_dir_all(root.path().join("hooks")).expect("hooks directory");
    std::fs::write(
        root.path().join("hooks/ready.yaml"),
        "id: ready\nenabled: true\n",
    )
    .expect("hook config");

    let entries = entries_by_key(
        scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("inventory"),
    );
    let missing = &entries[&("skill".to_owned(), "missing".to_owned())];
    assert_eq!(missing.availability, InventoryAvailability::Unavailable);
    let diagnostic = missing
        .diagnostic
        .as_deref()
        .expect("missing-file diagnostic");
    assert_eq!(
        diagnostic, "Managed authoring I/O failed: No such file or directory (os error 2)",
        "unexpected missing-instruction diagnosis: {diagnostic}"
    );
    let ready = &entries[&("hook".to_owned(), "ready".to_owned())];
    assert_eq!(ready.availability, InventoryAvailability::Available);
    assert_eq!(ready.relative_root, "hooks/ready.yaml");
}

#[test]
fn oversized_configuration_is_unavailable_without_hiding_valid_entries() {
    let root = tempfile::tempdir().expect("services root");
    std::fs::create_dir_all(root.path().join("artifacts")).expect("artifacts directory");
    let oversized = format!("id: large\npadding: {}\n", "x".repeat(65_537));
    std::fs::write(root.path().join("artifacts/large.yaml"), oversized).expect("large config");
    std::fs::write(
        root.path().join("artifacts/compact.json"),
        r#"{"id":"compact"}"#,
    )
    .expect("compact config");

    let entries = entries_by_key(
        scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("inventory"),
    );
    let large = &entries[&("artifact".to_owned(), "large".to_owned())];
    assert_eq!(large.availability, InventoryAvailability::Unavailable);
    assert!(
        large
            .diagnostic
            .as_deref()
            .expect("size diagnostic")
            .ends_with("Catalog configuration exceeds 64 KiB")
    );
    assert_eq!(
        entries[&("artifact".to_owned(), "compact".to_owned())].availability,
        InventoryAvailability::Available
    );
}

#[test]
fn hidden_and_unsupported_files_are_ignored_while_supported_files_are_projected() {
    let root = tempfile::tempdir().expect("services root");
    std::fs::create_dir_all(root.path().join("rules")).expect("rules directory");
    std::fs::write(root.path().join("rules/.hidden.yaml"), "id: hidden\n").expect("hidden config");
    std::fs::write(root.path().join("rules/notes.txt"), "not a catalog entry")
        .expect("unsupported file");
    std::fs::write(root.path().join("rules/policy.md"), "# policy\n").expect("markdown rule");
    std::fs::write(
        root.path().join("rules/data.yml"),
        "id: data\nenabled: true\n",
    )
    .expect("yaml rule");

    let entries = scan_configured_inventory(root.path(), &ServicesConfig::default())
        .expect("configured inventory");
    let keys = entries
        .iter()
        .map(|entry| entry.resource_key.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys, std::collections::BTreeSet::from(["data", "policy"]));
    assert!(entries.iter().all(|entry| {
        entry.kind == "rule" && entry.availability == InventoryAvailability::Available
    }));
}

#[cfg(unix)]
#[test]
fn skill_instruction_and_catalog_configuration_symlinks_are_unavailable() {
    let root = tempfile::tempdir().expect("services root");
    let external = tempfile::tempdir().expect("external files");
    std::fs::write(external.path().join("instruction.md"), "# external\n").expect("instruction");
    std::fs::write(external.path().join("plugin.yaml"), "id: linked\n").expect("plugin");

    std::fs::create_dir_all(root.path().join("skills/linked")).expect("skill directory");
    std::fs::write(
        root.path().join("skills/linked/config.yaml"),
        "id: linked\nfile: SKILL.md\n",
    )
    .expect("skill config");
    std::os::unix::fs::symlink(
        external.path().join("instruction.md"),
        root.path().join("skills/linked/SKILL.md"),
    )
    .expect("instruction symlink");

    std::fs::create_dir_all(root.path().join("plugins/linked")).expect("plugin directory");
    std::os::unix::fs::symlink(
        external.path().join("plugin.yaml"),
        root.path().join("plugins/linked/config.yaml"),
    )
    .expect("config symlink");

    let entries = entries_by_key(
        scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("inventory"),
    );
    assert!(
        entries[&("skill".to_owned(), "linked".to_owned())]
            .diagnostic
            .as_deref()
            .expect("skill diagnostic")
            .ends_with("Skill instruction file is a symlink")
    );
    assert!(
        entries[&("plugin".to_owned(), "linked".to_owned())]
            .diagnostic
            .as_deref()
            .expect("plugin diagnostic")
            .ends_with("Catalog configuration is a symlink")
    );
}

#[cfg(unix)]
#[test]
fn services_root_directory_symlink_is_resolved_but_file_symlink_is_rejected() {
    let target = tempfile::tempdir().expect("target services root");
    std::fs::create_dir_all(target.path().join("rules")).expect("rules directory");
    std::fs::write(target.path().join("rules/linked.md"), "# linked\n").expect("rule");
    let holder = tempfile::tempdir().expect("link holder");
    let root_link = holder.path().join("current");
    std::os::unix::fs::symlink(target.path(), &root_link).expect("root directory symlink");

    let entries = scan_configured_inventory(&root_link, &ServicesConfig::default())
        .expect("linked root inventory");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].relative_root, "rules/linked.md");
    assert_eq!(entries[0].availability, InventoryAvailability::Available);

    let file = holder.path().join("not-a-root");
    std::fs::write(&file, "not a directory").expect("file");
    let file_link = holder.path().join("file-link");
    std::os::unix::fs::symlink(&file, &file_link).expect("file symlink");
    let error = scan_configured_inventory(&file_link, &ServicesConfig::default())
        .expect_err("file root link must be rejected");
    assert!(
        error
            .to_string()
            .contains("Configured inventory root link does not name a directory")
    );
}

#[cfg(unix)]
#[test]
fn catalog_directory_symlink_and_non_utf8_name_abort_the_scan() {
    use std::os::unix::ffi::OsStringExt;

    let root = tempfile::tempdir().expect("services root");
    let external = tempfile::tempdir().expect("external catalog");
    std::os::unix::fs::symlink(external.path(), root.path().join("plugins"))
        .expect("catalog symlink");
    let error = scan_configured_inventory(root.path(), &ServicesConfig::default())
        .expect_err("catalog symlink must be rejected");
    assert!(
        error
            .to_string()
            .contains("Configured catalog is a symlink")
    );

    std::fs::remove_file(root.path().join("plugins")).expect("remove catalog link");
    std::fs::create_dir(root.path().join("plugins")).expect("plugins directory");
    let invalid_name = std::ffi::OsString::from_vec(vec![b'b', 0x80]);
    std::fs::write(
        root.path().join("plugins").join(invalid_name),
        "id: invalid\n",
    )
    .expect("non-UTF8 entry");
    let error = scan_configured_inventory(root.path(), &ServicesConfig::default())
        .expect_err("non-UTF8 names must be rejected");
    assert!(error.to_string().contains("Inventory names must be UTF-8"));
}
