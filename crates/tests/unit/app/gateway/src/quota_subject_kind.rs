use systemprompt_gateway::policies::{GatewayPolicySpec, QuotaWindow};
use systemprompt_identifiers::QuotaSubjectKind;

#[test]
fn a_rate_window_keyed_by_a_dimension_is_rejected_by_the_policy_file() {
    let err = serde_yaml::from_str::<GatewayPolicySpec>(
        r"
rate_windows:
  - window_seconds: 60
    subject: team
    max_requests: 5
",
    )
    .unwrap_err();
    assert!(err.to_string().contains("rate window subject"), "{err}");
    let spec: GatewayPolicySpec = serde_yaml::from_str(
        r"
rate_windows:
  - window_seconds: 60
    subject: api_key
    max_requests: 5
",
    )
    .unwrap();
    assert_eq!(spec.rate_windows[0].subject, QuotaSubjectKind::ApiKey);
}

#[test]
fn a_window_without_a_subject_is_keyed_by_the_user() {
    let window = QuotaWindow::default();
    assert_eq!(
        window.subject.parse::<QuotaSubjectKind>().unwrap(),
        QuotaSubjectKind::User
    );
}

#[test]
fn policy_yaml_subjects_resolve_to_principals_or_dimensions() {
    let spec: GatewayPolicySpec = serde_yaml::from_str(
        r"
quota_windows:
  - window_seconds: 60
    max_requests: 10
  - window_seconds: 60
    subject: api_key
    max_requests: 5
  - window_seconds: 3600
    subject: team
    max_requests: 100
",
    )
    .unwrap();
    let kinds: Vec<QuotaSubjectKind> = spec
        .quota_windows
        .iter()
        .map(|w| w.subject.parse().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            QuotaSubjectKind::User,
            QuotaSubjectKind::ApiKey,
            "team".parse::<QuotaSubjectKind>().unwrap(),
        ]
    );
}
