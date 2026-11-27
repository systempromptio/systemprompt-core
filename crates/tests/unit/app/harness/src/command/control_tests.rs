use systemprompt_engine::store::MessageId;
use systemprompt_harness::command::{Control, ControlDelivery};

const MESSAGE: &str = "msg_00000000000010000000000003";

#[test]
fn each_control_is_one_json_object_with_a_type() {
    assert_eq!(
        Control::parse(r#"{"type":"prompt","text":"also the tests"}"#).unwrap(),
        Control::Prompt {
            text: "also the tests".to_owned(),
            delivery: ControlDelivery::Steer,
        }
    );
    assert_eq!(
        Control::parse(r#"{"type":"prompt","text":"then this","delivery":"queue"}"#).unwrap(),
        Control::Prompt {
            text: "then this".to_owned(),
            delivery: ControlDelivery::Queue,
        }
    );
    assert_eq!(
        Control::parse(r#"{"type":"abort"}"#).unwrap(),
        Control::Abort
    );
    assert_eq!(
        Control::parse(r#"{"type":"close"}"#).unwrap(),
        Control::Close
    );
    assert_eq!(
        Control::parse(r#"{"type":"revert","message_id":"msg_00000000000010000000000003"}"#)
            .unwrap(),
        Control::Revert {
            message_id: MessageId::parse(MESSAGE).unwrap(),
        }
    );
}

#[test]
fn a_control_that_is_not_understood_is_refused() {
    for line in [
        "",
        "abort",
        r#"{"type":"pause"}"#,
        r#"{"type":"prompt"}"#,
        r#"{"type":"prompt","text":"x","delivery":"later"}"#,
        r#"{"type":"revert","message_id":"3"}"#,
        r#"{"type":"prompt","text":"x","now":true}"#,
    ] {
        assert!(Control::parse(line).is_err(), "{line}");
    }
}
