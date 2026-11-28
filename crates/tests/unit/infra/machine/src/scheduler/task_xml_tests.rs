use systemprompt_machine::scheduler::{EntryId, SchedulerEntry, Trigger, task_xml};

use super::{entry, every_half_hour};
use crate::support::{program, program_text};

#[test]
fn definition_escapes_text_and_quotes_arguments_for_the_command_line() {
    let xml = task_xml::render(&entry(Trigger::AtLogin));

    assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-16"?>"#));
    assert!(xml.contains("<Description>Example &lt;sync&gt; &amp; more</Description>"));
    assert!(xml.contains(&format!("<Command>{}</Command>", program_text())));
    assert!(xml.contains("<Arguments>sync --label &quot;two words&quot;</Arguments>"));
    assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
}

#[test]
fn trailing_backslash_before_a_closing_quote_is_doubled() {
    let entry = SchedulerEntry::new(
        EntryId::parse("example").expect("id"),
        "Example",
        &program(),
        vec![
            r"C:\dir with space\".to_owned(),
            r#"say "hi""#.to_owned(),
            String::new(),
        ],
        Trigger::OnDemand,
    )
    .expect("entry");

    let xml = task_xml::render(&entry);

    assert!(
        xml.contains(
            r"<Arguments>&quot;C:\dir with space\\&quot; &quot;say \&quot;hi\&quot;&quot; &quot;&quot;</Arguments>"
        ),
        "{xml}"
    );
}

#[test]
fn login_entry_has_a_logon_trigger_and_no_repetition() {
    let xml = task_xml::render(&entry(Trigger::AtLogin));
    assert!(xml.contains("<LogonTrigger>"));
    assert!(!xml.contains("<TimeTrigger>"));
}

#[test]
fn repeating_entry_runs_at_logon_and_at_its_interval() {
    let xml = task_xml::render(&entry(every_half_hour()));
    assert!(xml.contains("<LogonTrigger>"));
    assert!(xml.contains("<Interval>PT30M</Interval>"));
    assert!(xml.contains("<StartBoundary>"));
}

#[test]
fn on_demand_entry_has_no_trigger() {
    let xml = task_xml::render(&entry(Trigger::OnDemand));
    assert!(xml.contains("<Triggers />"));
    assert!(!xml.contains("<LogonTrigger>"));
    assert!(xml.contains("<AllowStartOnDemand>true</AllowStartOnDemand>"));
}

#[test]
fn encoding_is_utf_16_little_endian_with_a_byte_order_mark() {
    assert_eq!(
        task_xml::encode("<a>\u{e9}</a>"),
        [
            0xFF, 0xFE, b'<', 0, b'a', 0, b'>', 0, 0xE9, 0, b'<', 0, b'/', 0, b'a', 0, b'>', 0
        ]
    );
}
