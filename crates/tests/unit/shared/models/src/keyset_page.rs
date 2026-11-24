use chrono::{DateTime, Utc};
use systemprompt_identifiers::TaskId;
use systemprompt_models::api::{KeysetCursor, KeysetCursorError, KeysetPage};

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000 + seconds, 0).expect("timestamp")
}

fn key(row: &(i64, &'static str)) -> KeysetCursor<TaskId> {
    KeysetCursor::new(at(row.0), TaskId::new(row.1))
}

#[test]
fn a_short_page_has_no_cursor() {
    let page = KeysetPage::from_lookahead(vec![(3, "c"), (2, "b")], 2, key);
    assert_eq!(page.items, vec![(3, "c"), (2, "b")]);
    assert!(page.next_cursor.is_none());
}

#[test]
fn a_lookahead_row_is_dropped_and_names_the_last_kept_row() {
    let page = KeysetPage::from_lookahead(vec![(3, "c"), (2, "b"), (1, "a")], 2, key);
    assert_eq!(page.items, vec![(3, "c"), (2, "b")]);
    assert_eq!(page.next_cursor, Some(key(&(2, "b"))));
}

#[test]
fn a_typed_id_cursor_round_trips_opaquely() {
    let cursor = key(&(7, "task-7"));
    let wire = cursor.to_string();
    assert!(!wire.contains("task-7"), "{wire}");
    assert_eq!(
        wire.parse::<KeysetCursor<TaskId>>().expect("decodes"),
        cursor
    );
    let json = serde_json::to_value(&cursor).expect("serialises");
    assert_eq!(json, serde_json::json!(wire));
}

#[test]
fn an_empty_id_is_a_typed_decode_error() {
    use base64::Engine;
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("2026-10-01T00:00:00Z|");
    assert!(matches!(
        raw.parse::<KeysetCursor<TaskId>>(),
        Err(KeysetCursorError::Id(_))
    ));
}
