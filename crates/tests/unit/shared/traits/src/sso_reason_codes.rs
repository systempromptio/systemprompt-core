use systemprompt_traits::sso::{SsoError, SsoRefusal};

#[test]
fn reason_codes_are_pinned() {
    let table: Vec<(SsoError, &str, &str)> = vec![
        (
            SsoError::Unavailable("x".into()),
            "unavailable",
            "unavailable",
        ),
        (SsoError::internal("boom"), "error", "error"),
        (SsoError::NoSubject, "no_subject", "no_subject"),
        (SsoError::StateLost, "correlation_lost", "state_lost"),
        (
            SsoError::UnsolicitedNotAllowed,
            "correlation_lost",
            "unsolicited",
        ),
        (
            SsoError::InvalidAssertion("sig".into()),
            "invalid_assertion",
            "invalid_assertion",
        ),
        (SsoError::Replayed, "invalid_assertion", "replayed"),
        (SsoError::NoEmail, "no_email", "no_email"),
        (
            SsoError::Refused(SsoRefusal::DomainNotAllowed),
            "forbidden",
            "forbidden",
        ),
        (
            SsoError::Refused(SsoRefusal::GroupOverage),
            "group_overage",
            "group_overage",
        ),
        (
            SsoError::Refused(SsoRefusal::NoGroup),
            "no_group",
            "no_group",
        ),
        (
            SsoError::Refused(SsoRefusal::NotProvisioned),
            "not_provisioned",
            "not_provisioned",
        ),
    ];
    for (error, code, audit) in &table {
        assert_eq!(error.reason_code(), *code, "{error:?}");
        assert_eq!(error.audit_reason(), *audit, "{error:?}");
    }
}

#[test]
fn refusal_converts_into_error() {
    let error: SsoError = SsoRefusal::NoGroup.into();
    assert_eq!(error.reason_code(), "no_group");
}
