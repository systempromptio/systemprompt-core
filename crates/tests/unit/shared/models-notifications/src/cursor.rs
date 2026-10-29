use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{TimeZone, Utc};
use systemprompt_models::notifications::{KeysetCursor, KeysetCursorError};
use uuid::Uuid;

fn cursor() -> KeysetCursor {
    let at = Utc
        .timestamp_opt(1_761_700_000, 123_456_000)
        .single()
        .expect("timestamp");
    KeysetCursor::new(at, Uuid::new_v4())
}

#[test]
fn a_cursor_round_trips_through_its_opaque_string() {
    let original = cursor();
    let encoded = original.to_string();
    assert!(
        !encoded.contains('|') && !encoded.contains('='),
        "the string form is unpadded base64url: {encoded}"
    );
    assert_eq!(encoded.parse::<KeysetCursor>().expect("decodes"), original);
    let json = serde_json::to_value(original).expect("serialises");
    assert_eq!(json, serde_json::Value::String(encoded));
    assert_eq!(
        serde_json::from_value::<KeysetCursor>(json).expect("deserialises"),
        original
    );
}

#[test]
fn the_cursor_keeps_microsecond_precision() {
    let original = cursor();
    let decoded: KeysetCursor = original.to_string().parse().expect("decodes");
    assert_eq!(decoded.at.timestamp_subsec_micros(), 123_456);
}

#[test]
fn garbage_cursors_are_refused_with_the_failing_part() {
    assert!(matches!(
        "!!!".parse::<KeysetCursor>(),
        Err(KeysetCursorError::Encoding(_))
    ));
    let no_separator = URL_SAFE_NO_PAD.encode("2026-10-29T00:00:00Z");
    assert!(matches!(
        no_separator.parse::<KeysetCursor>(),
        Err(KeysetCursorError::Shape)
    ));
    let bad_time = URL_SAFE_NO_PAD.encode(format!("yesterday|{}", Uuid::new_v4()));
    assert!(matches!(
        bad_time.parse::<KeysetCursor>(),
        Err(KeysetCursorError::Timestamp(_))
    ));
    let bad_id = URL_SAFE_NO_PAD.encode("2026-10-29T00:00:00.000000Z|not-a-uuid");
    assert!(matches!(
        bad_id.parse::<KeysetCursor>(),
        Err(KeysetCursorError::Id(_))
    ));
    let not_utf8 = URL_SAFE_NO_PAD.encode([0xff_u8, 0xfe, 0xfd]);
    assert!(matches!(
        not_utf8.parse::<KeysetCursor>(),
        Err(KeysetCursorError::Utf8(_))
    ));
}
