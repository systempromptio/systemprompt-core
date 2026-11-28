use std::io::Cursor;

use systemprompt_engine::run::Reply;
use systemprompt_harness::command::{ControlCommand, ControlError, ExitStatus, read_controls};
use systemprompt_identifiers::engine::PermissionId;

const PERMISSION: &str = "per_00000000000010000000000003";

fn text(value: &str) -> String {
    value.to_owned()
}

#[test]
fn each_command_is_one_json_object_named_by_do() {
    assert_eq!(
        ControlCommand::parse(r#"{"do":"prompt","text":"then this"}"#).unwrap(),
        ControlCommand::Prompt {
            text: text("then this"),
        }
    );
    assert_eq!(
        ControlCommand::parse(r#"{"do":"steer","text":"also the tests"}"#).unwrap(),
        ControlCommand::Steer {
            text: text("also the tests"),
        }
    );
    assert_eq!(
        ControlCommand::parse(r#"{"do":"abort"}"#).unwrap(),
        ControlCommand::Abort {}
    );
    assert_eq!(
        ControlCommand::parse(r#"{"do":"revert","prompt":2}"#).unwrap(),
        ControlCommand::Revert { prompt: 2 }
    );
    assert_eq!(
        ControlCommand::parse(r#"{"do":"end"}"#).unwrap(),
        ControlCommand::End {}
    );
}

#[test]
fn a_reply_carries_the_request_id_and_the_engine_reply() {
    let id = PermissionId::parse(PERMISSION).unwrap();
    for (reply, expected) in [
        (r#"{"type":"once"}"#, Reply::Once),
        (r#"{"type":"always"}"#, Reply::Always),
        (r#"{"type":"reject"}"#, Reply::Reject { feedback: None }),
        (
            r#"{"type":"reject","feedback":"not this one"}"#,
            Reply::Reject {
                feedback: Some(text("not this one")),
            },
        ),
    ] {
        let line = format!(r#"{{"do":"reply","id":"{PERMISSION}","reply":{reply}}}"#);
        assert_eq!(
            ControlCommand::parse(&line).unwrap(),
            ControlCommand::Reply {
                id: id.clone(),
                reply: expected,
            },
            "{line}"
        );
    }
}

#[test]
fn a_command_with_a_key_it_does_not_know_is_refused() {
    for line in [
        r#"{"do":"prompt","text":"x","now":true}"#,
        r#"{"do":"steer","text":"x","delivery":"queue"}"#,
        r#"{"do":"abort","session":"s"}"#,
        r#"{"do":"revert","prompt":1,"message_id":"msg_00000000000010000000000003"}"#,
        r#"{"do":"reply","id":"per_00000000000010000000000003","reply":{"type":"once"},"x":1}"#,
        r#"{"do":"end","code":0}"#,
    ] {
        assert!(ControlCommand::parse(line).is_err(), "{line}");
    }
}

#[test]
fn a_command_that_is_not_understood_is_refused() {
    for line in [
        "",
        "abort",
        r#"{"type":"abort"}"#,
        r#"{"do":"pause"}"#,
        r#"{"do":"prompt"}"#,
        r#"{"do":"steer","text":3}"#,
        r#"{"do":"revert","prompt":-1}"#,
        r#"{"do":"revert","prompt":"first"}"#,
        r#"{"do":"reply","id":"3","reply":{"type":"once"}}"#,
        r#"{"do":"reply","id":"per_00000000000010000000000003"}"#,
        r#"{"do":"reply","id":"per_00000000000010000000000003","reply":"once"}"#,
        r#"{"do":"reply","id":"per_00000000000010000000000003","reply":{"type":"maybe"}}"#,
    ] {
        assert!(ControlCommand::parse(line).is_err(), "{line}");
    }
}

#[tokio::test]
async fn the_reader_feeds_the_commands_in_order_and_skips_blank_lines() {
    let input = Cursor::new(text(
        "{\"do\":\"steer\",\"text\":\"one\"}\n\n  \r\n{\"do\":\"abort\"}\n",
    ));

    let (mut feed, _reader) = read_controls(input);

    assert_eq!(
        feed.recv().await.unwrap().unwrap(),
        ControlCommand::Steer { text: text("one") }
    );
    assert_eq!(
        feed.recv().await.unwrap().unwrap(),
        ControlCommand::Abort {}
    );
    assert!(feed.recv().await.is_none());
}

#[tokio::test]
async fn the_reader_stops_at_end_and_leaves_the_rest_unread() {
    let input = Cursor::new(text(
        "{\"do\":\"end\"}\n{\"do\":\"prompt\",\"text\":\"late\"}\n",
    ));

    let (mut feed, _reader) = read_controls(input);

    assert_eq!(feed.recv().await.unwrap().unwrap(), ControlCommand::End {});
    assert!(feed.recv().await.is_none());
}

#[tokio::test]
async fn a_line_that_does_not_decode_is_named_by_its_number_and_ends_the_feed() {
    let input = Cursor::new(text(
        "{\"do\":\"abort\"}\n\n{\"do\":\"pause\"}\n{\"do\":\"end\"}\n",
    ));

    let (mut feed, _reader) = read_controls(input);

    assert_eq!(
        feed.recv().await.unwrap().unwrap(),
        ControlCommand::Abort {}
    );
    let error = feed.recv().await.unwrap().unwrap_err();
    assert!(matches!(error, ControlError::Undecodable { line: 3, .. }));
    assert_eq!(
        error.to_string(),
        "line 3 of the control stream does not decode"
    );
    assert!(feed.recv().await.is_none());
}

#[test]
fn a_control_error_ends_the_run_as_usage_unless_the_stream_broke() {
    let undecodable = ControlError::Undecodable {
        line: 1,
        source: ControlCommand::parse("{}").unwrap_err(),
    };
    assert_eq!(undecodable.status(), ExitStatus::Usage);
    assert_eq!(
        ControlError::NoSuchPrompt { prompt: 4 }.status(),
        ExitStatus::Usage
    );
    assert_eq!(
        ControlError::NoSuchPrompt { prompt: 4 }.to_string(),
        "the session has no user prompt number 4"
    );
    assert_eq!(
        ControlError::Unreadable(std::io::Error::other("closed")).status(),
        ExitStatus::Internal
    );
}
