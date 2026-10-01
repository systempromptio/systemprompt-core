use systemprompt_bridge::host_sync::{HostSyncReport, HostWarningKind};

#[test]
fn a_warning_travels_in_the_report_with_its_kind_and_host() {
    let mut report = HostSyncReport::ok();
    assert!(report.warnings.is_empty(), "a clean sync reports nothing");

    report.warn(
        HostWarningKind::ToolCatalog,
        "claude-desktop",
        "tool catalog not refreshed",
    );

    assert_eq!(report.warnings.len(), 1);
    let warning = &report.warnings[0];
    assert_eq!(warning.kind, HostWarningKind::ToolCatalog);
    assert_eq!(warning.host_id.as_str(), "claude-desktop");
    assert_eq!(warning.message, "tool catalog not refreshed");
}

#[test]
fn merging_reports_keeps_every_warning_in_order() {
    let mut first = HostSyncReport::ok();
    first.warn(HostWarningKind::Manifest, "claude-code", "first");
    let mut second = HostSyncReport::ok();
    second.warn(HostWarningKind::PermissionRules, "claude-code", "second");

    first.merge(second);

    let kinds: Vec<HostWarningKind> = first.warnings.iter().map(|w| w.kind).collect();
    assert_eq!(
        kinds,
        vec![HostWarningKind::Manifest, HostWarningKind::PermissionRules]
    );
}
