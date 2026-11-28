use std::path::Path;

use systemprompt_machine::scheduler::{EntryError, EntryId, Interval, SchedulerEntry, Trigger};

use crate::support::program;

fn id() -> EntryId {
    EntryId::parse("io.example.sync").expect("a valid id")
}

#[test]
fn id_accepts_labels_task_names_and_unit_names() {
    for value in ["io.example.sync", "ExampleSync", "example-sync_2"] {
        assert_eq!(EntryId::parse(value).expect("valid").as_str(), value);
    }
}

#[test]
fn id_refuses_anything_a_scheduler_would_read_as_syntax() {
    for value in [
        "",
        ".hidden",
        "-flag",
        "two words",
        "a/b",
        r"a\b",
        "unit@1",
        "caf\u{e9}",
    ] {
        assert_eq!(
            EntryId::parse(value),
            Err(EntryError::Id {
                value: value.to_owned()
            })
        );
    }
    assert!(EntryId::parse(&"a".repeat(121)).is_err());
}

#[test]
fn interval_runs_from_one_minute_to_thirty_one_days() {
    assert_eq!(
        Interval::minutes(0),
        Err(EntryError::Interval { minutes: 0 })
    );
    assert_eq!(Interval::minutes(1).expect("one minute").as_seconds(), 60);
    assert_eq!(
        Interval::minutes(44_640).expect("31 days").as_minutes(),
        44_640
    );
    assert_eq!(
        Interval::minutes(44_641),
        Err(EntryError::Interval { minutes: 44_641 })
    );
}

#[test]
fn entry_refuses_a_relative_program() {
    let outcome = SchedulerEntry::new(
        id(),
        "Sync",
        Path::new("example"),
        Vec::new(),
        Trigger::AtLogin,
    );
    assert!(matches!(outcome, Err(EntryError::Program { .. })));
}

#[test]
fn entry_refuses_a_description_that_spans_lines() {
    for description in ["", " ", "Sync\nExecStart=/bin/false"] {
        let outcome =
            SchedulerEntry::new(id(), description, &program(), Vec::new(), Trigger::AtLogin);
        assert_eq!(outcome, Err(EntryError::Description));
    }
}

#[test]
fn entry_names_the_argument_that_holds_a_control_character() {
    let outcome = SchedulerEntry::new(
        id(),
        "Sync",
        &program(),
        vec!["ok".to_owned(), "ok".to_owned(), "line\nbreak".to_owned()],
        Trigger::OnDemand,
    );
    assert_eq!(outcome, Err(EntryError::Argument { index: 2 }));
}
