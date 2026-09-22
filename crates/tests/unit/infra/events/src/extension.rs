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
    let tables: Vec<_> = schemas
        .iter()
        .filter(|schema| schema.table.is_some())
        .collect();
    assert_eq!(tables.len(), 1);
    let outbox = tables[0];
    assert_eq!(outbox.table.as_deref(), Some("event_outbox"));
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
    let migrations = EventsExtension.migrations();
    let names: Vec<&str> = migrations.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "actor_attribution",
            "actor_attribution_lock",
            "outbox_origin_instance",
            "durable_consumption",
            "reporting_privacy",
            "user_privacy_delivery",
            "drop_duplicate_actor_id_check"
        ],
        "every file in schema/migrations must be discovered by the build script, \
         in order, under its on-disk stem"
    );
}

#[test]
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = EventsExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION sp_capture_reporting_change()")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "outbox capture function must be installed before owner triggers"
    );
    assert!(capture[0].sql.contains("event_outbox_reporting_revision"));
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.begin_reporting_outbox_privacy")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
    assert!(privacy[0].sql.contains("reporting_privacy_changes"));
    assert!(privacy[0].sql.contains("acknowledge_reporting_privacy"));
    assert!(privacy[0].sql.contains("finish_reporting_outbox_privacy"));
}
