use systemprompt_identifiers::{CloudAppId, ElevatedJobId, TeamsAppId};

#[test]
fn teams_app_id_round_trips() {
    let id = TeamsAppId::try_new("00000000-0000-0000-0000-000000000001").unwrap();
    assert_eq!(id.as_str(), "00000000-0000-0000-0000-000000000001");
    let json = serde_json::to_string(&id).unwrap();
    let back: TeamsAppId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn teams_app_id_rejects_blank() {
    TeamsAppId::try_new("").unwrap_err();
    TeamsAppId::try_new(" ").unwrap_err();
    serde_json::from_str::<TeamsAppId>("\"\"").unwrap_err();
}

#[test]
fn cloud_app_id_round_trips() {
    let id: CloudAppId = "sp-tenant-app".parse().unwrap();
    assert_eq!(id, CloudAppId::new("sp-tenant-app"));
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"sp-tenant-app\"");
}

#[test]
fn cloud_app_id_rejects_blank() {
    CloudAppId::try_new("").unwrap_err();
    serde_json::from_str::<CloudAppId>("\"\"").unwrap_err();
}

#[test]
fn elevated_job_id_generate_is_uuid() {
    let id = ElevatedJobId::generate();
    let uuid = id.to_uuid().unwrap();
    assert_eq!(ElevatedJobId::from_uuid(uuid), id);
    assert_ne!(ElevatedJobId::generate(), id);
}

#[test]
fn elevated_job_id_rejects_non_uuid() {
    ElevatedJobId::try_new("job-1").unwrap_err();
    serde_json::from_str::<ElevatedJobId>("\"job-1\"").unwrap_err();
}

#[test]
fn elevated_job_id_serialises_like_a_uuid() {
    let uuid = uuid::Uuid::new_v4();
    let id = ElevatedJobId::from_uuid(uuid);
    assert_eq!(
        serde_json::to_string(&id).unwrap(),
        serde_json::to_string(&uuid).unwrap()
    );
}
