use std::str::FromStr;
use systemprompt_identifiers::{QuotaSubjectKind, ScopeDimension};

#[test]
fn principals_round_trip_and_dimensions_keep_their_name() {
    assert_eq!(
        QuotaSubjectKind::from_str("user").unwrap(),
        QuotaSubjectKind::User
    );
    assert_eq!(
        QuotaSubjectKind::from_str("api_key").unwrap(),
        QuotaSubjectKind::ApiKey
    );
    let team = QuotaSubjectKind::from_str("team").unwrap();
    assert_eq!(
        team,
        QuotaSubjectKind::Dimension(ScopeDimension::try_new("team").unwrap())
    );
    for kind in [QuotaSubjectKind::User, QuotaSubjectKind::ApiKey, team] {
        assert_eq!(QuotaSubjectKind::from_str(kind.as_str()).unwrap(), kind);
        assert_eq!(kind.to_string(), kind.as_str());
    }
}

#[test]
fn only_user_and_api_key_are_principals() {
    assert!(QuotaSubjectKind::User.is_principal());
    assert!(QuotaSubjectKind::ApiKey.is_principal());
    assert!(
        !QuotaSubjectKind::from_str("organization")
            .unwrap()
            .is_principal()
    );
}

#[test]
fn an_empty_kind_is_an_error_and_the_default_is_user() {
    assert!(QuotaSubjectKind::from_str("").is_err());
    assert!(
        QuotaSubjectKind::from_str("Team Name").is_err(),
        "a dimension name obeys ScopeDimension validation"
    );
    assert_eq!(QuotaSubjectKind::default(), QuotaSubjectKind::User);
}

#[test]
fn serde_is_the_bare_string() {
    assert_eq!(
        serde_json::to_string(&QuotaSubjectKind::ApiKey).unwrap(),
        "\"api_key\""
    );
    let parsed: QuotaSubjectKind = serde_json::from_str("\"project\"").unwrap();
    assert_eq!(parsed, QuotaSubjectKind::from_str("project").unwrap());
    assert!(serde_json::from_str::<QuotaSubjectKind>("\"\"").is_err());
}
