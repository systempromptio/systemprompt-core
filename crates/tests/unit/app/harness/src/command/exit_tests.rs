use std::collections::BTreeSet;

use systemprompt_device::view::{SignInFailure, SyncErrorKind};
use systemprompt_engine::run::ErrorKind;
use systemprompt_engine::store::MessageErrorKind;
use systemprompt_harness::command::ExitStatus;

const ALL: [ExitStatus; 9] = [
    ExitStatus::Completed,
    ExitStatus::Failed,
    ExitStatus::NotSignedIn,
    ExitStatus::Unavailable,
    ExitStatus::Usage,
    ExitStatus::Internal,
    ExitStatus::Refused,
    ExitStatus::TimedOut,
    ExitStatus::Aborted,
];

#[test]
fn every_status_has_its_own_documented_code() {
    let codes: Vec<u8> = ALL.iter().map(|status| status.code()).collect();
    assert_eq!(codes, [0, 1, 5, 10, 64, 70, 77, 124, 130]);
    assert_eq!(codes.iter().collect::<BTreeSet<_>>().len(), ALL.len());
}

#[test]
fn only_completed_is_success() {
    for status in ALL {
        assert_eq!(status.is_success(), status == ExitStatus::Completed);
    }
}

#[test]
fn a_status_serialises_as_its_snake_case_name() {
    let names: Vec<String> = ALL
        .iter()
        .map(|status| {
            serde_json::to_value(status)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "completed",
            "failed",
            "not_signed_in",
            "unavailable",
            "usage",
            "internal",
            "refused",
            "timed_out",
            "aborted"
        ]
    );
}

#[test]
fn engine_error_kinds_map_to_statuses() {
    let cases = [
        (ErrorKind::NotSignedIn, ExitStatus::NotSignedIn),
        (ErrorKind::ModelNotOffered, ExitStatus::Refused),
        (ErrorKind::Governance, ExitStatus::Refused),
        (ErrorKind::InvalidInput, ExitStatus::Usage),
        (ErrorKind::Unavailable, ExitStatus::Unavailable),
        (ErrorKind::Aborted, ExitStatus::Aborted),
        (ErrorKind::Model, ExitStatus::Failed),
        (ErrorKind::ContextOverflow, ExitStatus::Failed),
        (ErrorKind::NotFound, ExitStatus::Internal),
        (ErrorKind::Busy, ExitStatus::Internal),
        (ErrorKind::Config, ExitStatus::Internal),
        (ErrorKind::Storage, ExitStatus::Internal),
        (ErrorKind::Workspace, ExitStatus::Internal),
        (ErrorKind::Internal, ExitStatus::Internal),
    ];
    for (kind, status) in cases {
        assert_eq!(ExitStatus::of_engine(kind), status, "{kind:?}");
    }
}

#[test]
fn stored_message_errors_map_to_statuses() {
    let cases = [
        (MessageErrorKind::Aborted, ExitStatus::Aborted),
        (MessageErrorKind::Auth, ExitStatus::NotSignedIn),
        (MessageErrorKind::Governance, ExitStatus::Refused),
        (MessageErrorKind::OutputLength, ExitStatus::Failed),
        (MessageErrorKind::ContextOverflow, ExitStatus::Failed),
        (MessageErrorKind::ContentFilter, ExitStatus::Failed),
        (MessageErrorKind::Api, ExitStatus::Failed),
        (MessageErrorKind::Unknown, ExitStatus::Failed),
    ];
    for (kind, status) in cases {
        assert_eq!(ExitStatus::of_message(kind), status, "{kind:?}");
    }
}

#[test]
fn sync_and_sign_in_failures_map_to_statuses() {
    let sync = [
        (SyncErrorKind::Unreachable, ExitStatus::Unavailable),
        (SyncErrorKind::RateLimited, ExitStatus::Unavailable),
        (SyncErrorKind::Refused, ExitStatus::Refused),
        (SyncErrorKind::KeyChanged, ExitStatus::Refused),
        (SyncErrorKind::Invalid, ExitStatus::Refused),
        (SyncErrorKind::Insecure, ExitStatus::NotSignedIn),
        (SyncErrorKind::Service, ExitStatus::Internal),
    ];
    for (kind, status) in sync {
        assert_eq!(ExitStatus::of_sync(kind), status, "{kind:?}");
    }
    let sign_in = [
        (SignInFailure::Unreachable, ExitStatus::Unavailable),
        (SignInFailure::RateLimited, ExitStatus::Unavailable),
        (SignInFailure::Refused, ExitStatus::Refused),
        (SignInFailure::Incompatible, ExitStatus::Failed),
        (SignInFailure::Insecure, ExitStatus::Failed),
        (SignInFailure::Failed, ExitStatus::Failed),
    ];
    for (failure, status) in sign_in {
        assert_eq!(ExitStatus::of_sign_in(failure), status, "{failure:?}");
    }
}
