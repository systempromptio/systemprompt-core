use std::io;

use systemprompt_bridge::install::approval::ApprovalRefusal;

#[test]
fn a_refusal_survives_the_io_error_it_travels_in() {
    let err: io::Error = ApprovalRefusal::Declined.into();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(ApprovalRefusal::of(&err), Some(ApprovalRefusal::Declined));

    let needs = ApprovalRefusal::NeedsPrompt { reason: "use Repair" };
    let err: io::Error = needs.into();
    assert_eq!(ApprovalRefusal::of(&err), Some(needs));
    assert_eq!(err.to_string(), "use Repair");
}

#[test]
fn an_os_denial_or_a_lookalike_message_is_not_a_refusal() {
    let denied = io::Error::new(io::ErrorKind::PermissionDenied, "read-only file system");
    assert_eq!(ApprovalRefusal::of(&denied), None);

    let lookalike = io::Error::other("user cancelled the administrator authorization prompt");
    assert_eq!(
        ApprovalRefusal::of(&lookalike),
        None,
        "classification is by type, never by message text"
    );
}
