use systemprompt_bridge::takeover::{
    ControlRequest, FOCUS_PATH, HANDOFF_PATH, SecondLaunch, control_request, parse_control_request,
    second_launch,
};

const TOKEN: &str = "csrf-token-0123";

#[test]
fn a_newer_launch_takes_over_an_older_running_bridge() {
    assert_eq!(
        second_launch("0.80.0", Some("0.79.0")),
        SecondLaunch::TakeOver
    );
}

#[test]
fn a_same_or_older_launch_focuses_the_running_bridge() {
    assert_eq!(second_launch("0.79.0", Some("0.79.0")), SecondLaunch::Focus);
    assert_eq!(second_launch("0.78.0", Some("0.79.0")), SecondLaunch::Focus);
}

#[test]
fn a_running_bridge_with_no_recorded_version_counts_as_older() {
    assert_eq!(second_launch("0.80.0", None), SecondLaunch::TakeOver);
}

#[test]
fn a_well_formed_handoff_request_is_recognised() {
    let head = control_request(HANDOFF_PATH, TOKEN);
    assert_eq!(
        parse_control_request(&head, TOKEN),
        Some(ControlRequest::Handoff)
    );
}

#[test]
fn a_well_formed_focus_request_is_recognised() {
    let head = control_request(FOCUS_PATH, TOKEN);
    assert_eq!(
        parse_control_request(&head, TOKEN),
        Some(ControlRequest::Focus)
    );
}

#[test]
fn a_request_with_the_wrong_token_is_refused() {
    let head = control_request(HANDOFF_PATH, "other-token-0000");
    assert_eq!(parse_control_request(&head, TOKEN), None);
    let tokenless = format!("POST {HANDOFF_PATH} HTTP/1.1\r\n\r\n");
    assert_eq!(parse_control_request(&tokenless, TOKEN), None);
}

#[test]
fn a_get_or_an_unknown_path_is_refused() {
    let get = format!("GET {HANDOFF_PATH}?t={TOKEN} HTTP/1.1\r\n\r\n");
    assert_eq!(parse_control_request(&get, TOKEN), None);
    let other = control_request("/api/quit", TOKEN);
    assert_eq!(parse_control_request(&other, TOKEN), None);
    assert_eq!(parse_control_request("", TOKEN), None);
}

#[test]
fn an_unparseable_version_on_either_side_focuses_so_two_builds_never_hand_over_to_each_other() {
    assert_eq!(
        second_launch("0.80.0", Some("not-a-version")),
        SecondLaunch::Focus
    );
    assert_eq!(
        second_launch("not-a-version", Some("0.79.0")),
        SecondLaunch::Focus
    );
    assert_eq!(
        second_launch("not-a-version", Some("not-a-version")),
        SecondLaunch::Focus
    );
    let versions = ["0.79.0", "0.80.0", "0.80.0-rc.1", "garbage", ""];
    for a in versions {
        for b in versions {
            let both = second_launch(a, Some(b)) == SecondLaunch::TakeOver
                && second_launch(b, Some(a)) == SecondLaunch::TakeOver;
            assert!(!both, "{a} and {b} would hand over to each other");
        }
    }
}
