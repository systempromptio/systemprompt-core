use systemprompt_machine::scheduler::{EntryId, SchedulerEntry, Trigger, systemd};

use super::{entry, every_half_hour};
use crate::support::{program, program_text};

#[test]
fn units_are_named_for_the_entry() {
    let id = EntryId::parse("io.example.sync").expect("id");
    assert_eq!(systemd::service_name(&id), "io.example.sync.service");
    assert_eq!(systemd::timer_name(&id), "io.example.sync.timer");
}

#[test]
fn login_entry_is_a_service_wanted_by_the_default_target() {
    let units = systemd::render(&entry(Trigger::AtLogin));

    assert_eq!(units.timer, None);
    assert!(
        units
            .service
            .contains("Description=Example <sync> & more\n")
    );
    assert!(units.service.contains(&format!(
        "ExecStart=\"{}\" \"sync\" \"--label\" \"two words\"\n",
        program_text().replace('\\', r"\\")
    )));
    assert!(units.service.contains("Type=simple\n"));
    assert!(
        units
            .service
            .ends_with("[Install]\nWantedBy=default.target\n")
    );
}

#[test]
fn repeating_entry_is_a_oneshot_service_started_by_a_timer() {
    let units = systemd::render(&entry(every_half_hour()));

    assert!(units.service.contains("Type=oneshot\n"));
    assert!(!units.service.contains("[Install]"));
    let timer = units.timer.expect("a repeating entry has a timer");
    assert!(timer.contains("OnUnitActiveSec=30min\n"));
    assert!(timer.contains("Unit=io.example.sync.service\n"));
    assert!(timer.ends_with("[Install]\nWantedBy=timers.target\n"));
}

#[test]
fn on_demand_entry_is_a_service_nothing_starts() {
    let units = systemd::render(&entry(Trigger::OnDemand));
    assert_eq!(units.timer, None);
    assert!(!units.service.contains("[Install]"));
}

#[test]
fn specifiers_variables_quotes_and_backslashes_reach_the_program_literally() {
    let entry = SchedulerEntry::new(
        EntryId::parse("example").expect("id"),
        "100% done",
        &program(),
        vec![r#"50% of $HOME "quoted" back\slash"#.to_owned()],
        Trigger::OnDemand,
    )
    .expect("entry");

    let units = systemd::render(&entry);

    assert!(units.service.contains("Description=100%% done\n"));
    assert!(
        units
            .service
            .contains(r#""50%% of $$HOME \"quoted\" back\\slash""#),
        "{}",
        units.service
    );
}
