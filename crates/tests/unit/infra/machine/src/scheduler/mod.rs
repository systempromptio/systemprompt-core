//! Tests for `systemprompt_machine::scheduler`, through its public API.

mod entry_tests;
mod launchd_tests;
mod systemd_tests;
mod task_xml_tests;

use systemprompt_machine::scheduler::{EntryId, Interval, SchedulerEntry, Trigger};

use crate::support::program;

pub(crate) fn entry(trigger: Trigger) -> SchedulerEntry {
    SchedulerEntry::new(
        EntryId::parse("io.example.sync").expect("a valid id"),
        "Example <sync> & more",
        &program(),
        vec![
            "sync".to_owned(),
            "--label".to_owned(),
            "two words".to_owned(),
        ],
        trigger,
    )
    .expect("a valid entry")
}

pub(crate) fn every_half_hour() -> Trigger {
    Trigger::Every(Interval::minutes(30).expect("a valid interval"))
}
