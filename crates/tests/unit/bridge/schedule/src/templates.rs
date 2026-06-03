use std::path::Path;
use systemprompt_bridge::schedule::{self, Os};

const BINARY: &str = "/opt/sp/systemprompt-bridge";

fn binary() -> &'static Path {
    Path::new(BINARY)
}

#[test]
fn parse_mac_aliases() {
    assert!(matches!(Os::parse("macos"), Some(Os::Mac)));
    assert!(matches!(Os::parse("darwin"), Some(Os::Mac)));
    assert!(matches!(Os::parse("mac"), Some(Os::Mac)));
}

#[test]
fn parse_windows_aliases() {
    assert!(matches!(Os::parse("windows"), Some(Os::Windows)));
    assert!(matches!(Os::parse("win"), Some(Os::Windows)));
}

#[test]
fn parse_linux_alias() {
    assert!(matches!(Os::parse("linux"), Some(Os::Linux)));
}

#[test]
fn parse_is_case_insensitive() {
    assert!(matches!(Os::parse("MacOS"), Some(Os::Mac)));
    assert!(matches!(Os::parse("DARWIN"), Some(Os::Mac)));
    assert!(matches!(Os::parse("MAC"), Some(Os::Mac)));
    assert!(matches!(Os::parse("WIN"), Some(Os::Windows)));
    assert!(matches!(Os::parse("Windows"), Some(Os::Windows)));
    assert!(matches!(Os::parse("Linux"), Some(Os::Linux)));
    assert!(matches!(Os::parse("LINUX"), Some(Os::Linux)));
}

#[test]
fn parse_unknown_returns_none() {
    assert!(Os::parse("freebsd").is_none());
    assert!(Os::parse("").is_none());
    assert!(Os::parse("solaris").is_none());
}

#[test]
fn current_returns_a_known_os() {
    let os = Os::current();
    assert!(matches!(os, Os::Mac | Os::Windows | Os::Linux));
}

#[test]
fn mac_template_substitutes_binary() {
    let rendered = schedule::template(Os::Mac, binary());
    assert!(rendered.contains(BINARY));
    assert!(!rendered.contains("{binary}"));
}

#[test]
fn windows_template_substitutes_binary() {
    let rendered = schedule::template(Os::Windows, binary());
    assert!(rendered.contains(BINARY));
    assert!(!rendered.contains("{binary}"));
}

#[test]
fn linux_template_substitutes_binary() {
    let rendered = schedule::template(Os::Linux, binary());
    assert!(rendered.contains(BINARY));
    assert!(!rendered.contains("{binary}"));
}

#[test]
fn mac_template_has_launchd_marker() {
    let rendered = schedule::template(Os::Mac, binary());
    assert!(rendered.contains("<plist version=\"1.0\">"));
    assert!(rendered.contains("io.systemprompt.bridge-sync"));
}

#[test]
fn windows_template_has_task_scheduler_marker() {
    let rendered = schedule::template(Os::Windows, binary());
    assert!(rendered.contains("<Task version=\"1.4\""));
    assert!(rendered.contains("schemas.microsoft.com"));
}

#[test]
fn linux_template_has_systemd_marker() {
    let rendered = schedule::template(Os::Linux, binary());
    assert!(rendered.contains("[Service]"));
    assert!(rendered.contains("WantedBy=timers.target"));
}

#[test]
fn template_filenames_per_os() {
    assert_eq!(
        schedule::template_filename(Os::Mac),
        "io.systemprompt.bridge-sync.plist"
    );
    assert_eq!(
        schedule::template_filename(Os::Windows),
        "systemprompt-bridge-sync.xml"
    );
    assert_eq!(
        schedule::template_filename(Os::Linux),
        "systemprompt-bridge-sync.service+timer"
    );
}

#[test]
fn mac_install_hint_mentions_launchctl() {
    let hint = schedule::install_hint(Os::Mac);
    assert!(!hint.is_empty());
    assert!(hint.contains("launchctl"));
}

#[test]
fn windows_install_hint_mentions_schtasks() {
    let hint = schedule::install_hint(Os::Windows);
    assert!(!hint.is_empty());
    assert!(hint.contains("schtasks"));
}

#[test]
fn linux_install_hint_mentions_systemctl() {
    let hint = schedule::install_hint(Os::Linux);
    assert!(!hint.is_empty());
    assert!(hint.contains("systemctl"));
}
