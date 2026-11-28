use systemprompt_machine::scheduler::{EntryId, Trigger, launchd};

use super::{entry, every_half_hour};
use crate::support::program_text;

#[test]
fn agent_file_is_named_for_the_label() {
    let id = EntryId::parse("io.example.sync").expect("id");
    assert_eq!(launchd::file_name(&id), "io.example.sync.plist");
}

#[test]
fn agent_carries_the_label_and_one_string_per_argument() {
    let plist = launchd::render(&entry(Trigger::AtLogin));

    assert!(plist.contains("<key>Label</key>\n    <string>io.example.sync</string>"));
    assert!(plist.contains(&format!(
        "<array>\n        <string>{}</string>\n        <string>sync</string>\n        <string>--label</string>\n        <string>two words</string>\n    </array>",
        program_text()
    )));
    assert!(plist.trim_end().ends_with("</plist>"));
}

#[test]
fn login_entry_runs_at_load_without_an_interval() {
    let plist = launchd::render(&entry(Trigger::AtLogin));
    assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
    assert!(!plist.contains("StartInterval"));
}

#[test]
fn repeating_entry_gives_its_interval_in_seconds() {
    let plist = launchd::render(&entry(every_half_hour()));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<key>StartInterval</key>\n    <integer>1800</integer>"));
}

#[test]
fn on_demand_entry_neither_runs_at_load_nor_repeats() {
    let plist = launchd::render(&entry(Trigger::OnDemand));
    assert!(!plist.contains("RunAtLoad"));
    assert!(!plist.contains("StartInterval"));
}
