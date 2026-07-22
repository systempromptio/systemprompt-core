use systemprompt_events::EventsExtension;
use systemprompt_extension::Extension;

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
    let schemas = EventsExtension.schemas();
    assert_eq!(schemas.len(), 1);

    let outbox = &schemas[0];
    assert_eq!(outbox.table, "event_outbox");
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
    ] {
        assert!(
            outbox.required_columns.iter().any(|c| c == column),
            "missing required column: {column}"
        );
    }
}

#[test]
fn migrations_come_from_the_schema_migrations_directory() {
    let migrations = EventsExtension.migrations();
    assert_eq!(
        migrations.len(),
        2,
        "both actor-attribution migrations must be discovered by the build script"
    );
    assert!(
        migrations
            .iter()
            .any(|m| m.name.contains("actor_attribution")),
        "migration names must carry the on-disk file stem"
    );
}
