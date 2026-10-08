use std::io;

use systemprompt_bridge::install::approval::{ApprovalRefusal, ElevationFailure, GatedChangeError};

#[test]
fn a_refusal_is_its_own_variant_and_keeps_its_reason() {
    let err = GatedChangeError::from(ApprovalRefusal::Declined);
    assert_eq!(err.refusal(), Some(ApprovalRefusal::Declined));

    let needs = ApprovalRefusal::NeedsPrompt {
        reason: "use Repair",
    };
    let err = GatedChangeError::from(needs);
    assert_eq!(err.refusal(), Some(needs));
    assert_eq!(err.to_string(), "use Repair");
}

#[test]
fn an_os_denial_or_a_lookalike_message_is_not_a_refusal() {
    let denied = GatedChangeError::from(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "read-only file system",
    ));
    assert_eq!(denied.refusal(), None);

    let lookalike = GatedChangeError::from(io::Error::other(
        "user cancelled the administrator authorization prompt",
    ));
    assert_eq!(
        lookalike.refusal(),
        None,
        "classification is by type, never by message text"
    );
}

#[test]
fn an_unverified_elevated_change_is_a_failure_not_a_refusal() {
    let err = GatedChangeError::from(ElevationFailure::Unverified(
        "elevated receipt lacks install C:\\policy".to_owned(),
    ));
    assert_eq!(err.refusal(), None);
}
