use systemprompt_events::EventsExtension;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Extension, StandingTag};
use systemprompt_test_fixtures::admitted_member;

fn contributions() -> ContributionSink {
    admitted_member(&EventsExtension, StandingTag::Core)
        .expect("events contributes")
        .sink
}

#[test]
fn metadata_identifies_the_events_extension() {
    let metadata = EventsExtension.metadata();
    assert_eq!(metadata.id, "events");
    assert_eq!(metadata.name, "Events");
    assert!(
        metadata.version.contains('.'),
        "version should be dotted semver: {}",
        metadata.version
    );
}

#[test]
fn schema_declares_event_outbox_with_relay_columns() {
    let sink = contributions();
    let tables: Vec<_> = sink
        .schemas()
        .iter()
        .filter(|schema| schema.table.is_some())
        .collect();
    assert_eq!(tables.len(), 2);
    let outbox = tables[0];
    assert_eq!(outbox.table.as_deref(), Some("event_outbox"));
    assert_eq!(tables[1].table.as_deref(), Some("event_outbox_backlog"));
    assert!(
        outbox.sql.contains("event_outbox"),
        "embedded DDL must create the declared table"
    );
    for column in [
        "id",
        "channel",
        "user_id",
        "payload",
        "actor_kind",
        "actor_id",
        "created_at",
        "consumer",
        "fact",
        "processed_at",
        "deliver_to_origin",
    ] {
        assert!(
            outbox.required_columns.iter().any(|c| c == column),
            "missing required column: {column}"
        );
    }
}

#[test]
fn migrations_come_from_the_schema_migrations_directory() {
    let sink = contributions();
    let names: Vec<&str> = sink.migrations().iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "actor_attribution",
            "actor_attribution_lock",
            "outbox_origin_instance",
            "durable_consumption",
            "reporting_privacy",
            "user_privacy_delivery",
            "drop_duplicate_actor_id_check",
            "restore_actor_id_nonempty",
            "retire_reporting_capture",
            "outbox_backlog",
            "outbox_backlog_serialised",
            "outbox_aggregate_order",
        ],
        "every file in schema/migrations must be discovered by the build script, \
         in order, under its on-disk stem"
    );
}
