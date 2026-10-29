use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::AudienceSubject;

#[test]
fn everyone_roles_users_and_dimensions_parse_to_their_variants() {
    assert_eq!(
        "everyone".parse::<AudienceSubject>().expect("parses"),
        AudienceSubject::Everyone
    );
    assert_eq!(
        "role:admin".parse::<AudienceSubject>().expect("parses"),
        AudienceSubject::Role("admin".to_owned())
    );
    assert_eq!(
        "group:sales".parse::<AudienceSubject>().expect("parses"),
        AudienceSubject::Dimension {
            kind: "group".to_owned(),
            value: "sales".to_owned()
        }
    );
    assert_eq!(
        "user:alice".parse::<AudienceSubject>().expect("parses"),
        AudienceSubject::User(UserId::new("alice"))
    );
}

#[test]
fn audience_subjects_display_as_the_form_they_parse_from() {
    for text in [
        "everyone",
        "role:x",
        "group:sales",
        "user:alice",
        "region:emea:north",
    ] {
        let subject: AudienceSubject = text.parse().expect("parses");
        assert_eq!(subject.to_string(), text);
        let json = serde_json::to_value(&subject).expect("serialises");
        assert_eq!(json, serde_json::Value::String(text.to_owned()));
        assert_eq!(
            serde_json::from_value::<AudienceSubject>(json).expect("deserialises"),
            subject
        );
    }
}

#[test]
fn a_subject_without_a_kind_or_value_is_refused() {
    for bad in ["", "admin", ":x", "role:", "Everyone"] {
        assert!(
            bad.parse::<AudienceSubject>().is_err(),
            "{bad:?} must not parse"
        );
    }
    assert!(serde_json::from_str::<AudienceSubject>("\"role:\"").is_err());
}
