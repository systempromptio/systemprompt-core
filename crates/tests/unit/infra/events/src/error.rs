use systemprompt_events::{EventError, EventResult};

#[test]
fn serialization_variant_display() {
    let json_err = serde_json::from_str::<serde_json::Value>("not-json").unwrap_err();
    let err = EventError::Serialization(json_err);
    let msg = err.to_string();
    assert!(msg.contains("event serialization failed"));
}

#[test]
fn channel_full_variant_display() {
    let err = EventError::ChannelFull {
        target: "user-abc".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("event channel saturated for"));
    assert!(msg.contains("user-abc"));
}

#[test]
fn event_error_debug() {
    let err = EventError::ChannelFull {
        target: "some-user".to_string(),
    };
    let debug = format!("{:?}", err);
    assert!(debug.contains("ChannelFull"));
}

#[test]
fn event_result_ok_is_ok() {
    let result: EventResult<u32> = Ok(42);
    assert!(result.is_ok());
}

#[test]
fn event_result_err_is_err() {
    let result: EventResult<u32> = Err(EventError::ChannelFull {
        target: "user".to_string(),
    });
    assert!(result.is_err());
}

#[test]
fn serialization_error_from_serde() {
    let json_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
    let err: EventError = json_err.into();
    let msg = err.to_string();
    assert!(msg.contains("event serialization failed"));
}

#[test]
fn channel_full_target_preserved() {
    let target = "conn-user-42".to_string();
    let err = EventError::ChannelFull {
        target: target.clone(),
    };
    let msg = err.to_string();
    assert!(msg.contains(&target));
}
