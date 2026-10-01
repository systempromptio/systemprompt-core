use systemprompt_bridge::ids::DeploymentOrganizationUuid;

const ORG: &str = "6f1c2a9e-3b4d-4e5f-8a7b-9c0d1e2f3a4b";

#[test]
fn deployment_organization_uuid_accepts_a_hyphenated_uuid() {
    let id = DeploymentOrganizationUuid::try_new(ORG).expect("hyphenated UUID is valid");
    assert_eq!(id.as_str(), ORG);
}

#[test]
fn deployment_organization_uuid_rejects_other_uuid_spellings() {
    assert!(DeploymentOrganizationUuid::try_new(ORG.replace('-', "")).is_err());
    assert!(DeploymentOrganizationUuid::try_new(format!("{{{ORG}}}")).is_err());
    assert!(DeploymentOrganizationUuid::try_new("not-a-uuid").is_err());
    assert!(DeploymentOrganizationUuid::try_new("").is_err());
}

#[test]
fn deployment_organization_uuid_deserialize_validates() {
    let json = format!("\"{ORG}\"");
    let id: DeploymentOrganizationUuid = serde_json::from_str(&json).expect("valid");
    assert_eq!(id.as_str(), ORG);
    assert!(serde_json::from_str::<DeploymentOrganizationUuid>("\"nope\"").is_err());
}
