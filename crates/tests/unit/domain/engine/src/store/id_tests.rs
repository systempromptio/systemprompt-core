use std::collections::HashSet;

use systemprompt_engine::store::{IdClock, MessageId, PartId, SessionId, now_ms};

use crate::store::support::TestResult;

#[test]
fn ids_are_prefixed_unique_and_ascending() -> TestResult {
    let clock = IdClock::new();
    let ids: Vec<MessageId> = (0..5000).map(|_| MessageId::mint(&clock)).collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    let unique: HashSet<&MessageId> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());

    let first = &ids[0];
    assert!(first.as_str().starts_with("msg_"));
    assert_eq!(MessageId::parse(first.as_str())?, *first);
    assert!((first.created_at_ms() - now_ms()).abs() < 60_000);
    assert!(SessionId::parse(first.as_str()).is_err());
    assert!(MessageId::parse("msg_short").is_err());

    let encoded = serde_json::to_string(first)?;
    let decoded: MessageId = serde_json::from_str(&encoded)?;
    assert_eq!(&decoded, first);
    assert!(serde_json::from_str::<PartId>(&encoded).is_err());
    Ok(())
}
