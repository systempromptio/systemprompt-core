use super::*;

#[test]
fn scanner_retains_malformed_entries() {
    let root = tempfile::tempdir().expect("root");
    std::fs::create_dir_all(root.path().join("skills/broken")).expect("dir");
    std::fs::write(root.path().join("skills/broken/config.yaml"), "[malformed").expect("config");
    let entries = scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("scan");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].availability, InventoryAvailability::Unavailable);
}

#[cfg(unix)]
#[test]
fn symlink_catalog_entry_is_explicitly_unavailable() {
    let root = tempfile::tempdir().expect("root");
    let external = tempfile::tempdir().expect("external");
    std::fs::create_dir(root.path().join("skills")).expect("skills");
    std::os::unix::fs::symlink(external.path(), root.path().join("skills/linked"))
        .expect("symlink");
    let entries = scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("scan");
    assert_eq!(entries[0].availability, InventoryAvailability::Unavailable);
    assert!(
        entries[0]
            .diagnostic
            .as_ref()
            .expect("diagnostic")
            .contains("symlink")
    );
}

#[test]
fn configured_marketplace_is_included_without_inventing_historical_plugin_membership() {
    let root = tempfile::tempdir().expect("root");
    let directory = root.path().join("marketplaces/team");
    std::fs::create_dir_all(&directory).expect("directory");
    std::fs::write(
        directory.join("config.yaml"),
        "id: team\nenabled: true\nplugins: [shared]\n",
    )
    .expect("config");
    let entries = scan_configured_inventory(root.path(), &ServicesConfig::default()).expect("scan");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, "marketplace");
    assert_eq!(entries[0].resource_key, "team");
    assert_eq!(entries[0].relative_root, "marketplaces/team");
}
