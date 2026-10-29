use systemprompt_models::notifications::NotificationKind;

#[test]
fn dotted_lowercase_kinds_are_accepted() {
    for kind in [
        NotificationKind::MENTIONED,
        NotificationKind::SHARED,
        NotificationKind::STREAM_PUBLISHED,
        "deal.stage_changed",
        "x1",
    ] {
        let parsed = NotificationKind::new(kind).expect("valid kind");
        assert_eq!(parsed.as_str(), kind);
        assert_eq!(
            parsed
                .to_string()
                .parse::<NotificationKind>()
                .expect("parses"),
            parsed
        );
    }
}

#[test]
fn malformed_kinds_are_refused_with_a_reason() {
    for (kind, reason) in [
        ("", "must not be empty"),
        ("document..shared", "segments must not be empty"),
        (".shared", "segments must not be empty"),
        (
            "Document.shared",
            "must contain only lowercase letters, digits, '_' and '.'",
        ),
        (
            "document-shared",
            "must contain only lowercase letters, digits, '_' and '.'",
        ),
    ] {
        let error = NotificationKind::new(kind).expect_err("invalid kind");
        assert_eq!(error.reason, reason, "{kind:?}");
    }
    assert!(serde_json::from_str::<NotificationKind>("\"Bad Kind\"").is_err());
}
