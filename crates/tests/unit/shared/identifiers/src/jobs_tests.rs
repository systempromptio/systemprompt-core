use systemprompt_identifiers::{DbValue, JobName, ScheduledJobId, ToDbValue};

#[test]
fn scheduled_job_id_generate_uuid_format() {
    let id = ScheduledJobId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(id.as_str().chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn scheduled_job_id_generate_unique() {
    let id1 = ScheduledJobId::generate();
    let id2 = ScheduledJobId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn scheduled_job_id_serde_transparent() {
    let id = ScheduledJobId::new("6f1c2a52-6b7e-4d3a-9c1e-2f4b8a9d0e11");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"6f1c2a52-6b7e-4d3a-9c1e-2f4b8a9d0e11\"");
    let deserialized: ScheduledJobId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn scheduled_job_id_rejects_non_uuid() {
    ScheduledJobId::try_new("job-1").unwrap_err();
    serde_json::from_str::<ScheduledJobId>("\"job-1\"").unwrap_err();
}

#[test]
fn scheduled_job_id_round_trips_through_uuid() {
    let id = ScheduledJobId::generate();
    let uuid = id.to_uuid().unwrap();
    assert_eq!(ScheduledJobId::from_uuid(uuid), id);
}

#[test]
fn scheduled_job_id_to_db_value_owned_and_ref() {
    let id = ScheduledJobId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}

#[test]
fn job_name_accepts_descriptive_names() {
    let name = JobName::new("daily-cleanup-expired-sessions");
    assert_eq!(name.as_str(), "daily-cleanup-expired-sessions");
}

#[test]
fn job_name_display_format() {
    let name = JobName::new("my-job");
    assert_eq!(format!("{}", name), "my-job");
}

#[test]
fn job_name_serde_transparent() {
    let name = JobName::new("serde-job");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"serde-job\"");
    let deserialized: JobName = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, name);
}

#[test]
fn job_name_try_new_and_parse_equal() {
    let a = JobName::try_new("x").unwrap();
    let b: JobName = "x".parse().unwrap();
    assert_eq!(a, b);
}

#[test]
fn job_name_rejects_empty() {
    JobName::try_new("").unwrap_err();
    serde_json::from_str::<JobName>("\"\"").unwrap_err();
}

#[test]
fn job_name_to_db_value_owned_and_ref() {
    let name = JobName::new("db");
    assert!(matches!(name.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&name).to_db_value(), DbValue::String(ref s) if s == "db"));
}

#[test]
fn job_name_into_string() {
    let s: String = JobName::new("convert").into();
    assert_eq!(s, "convert");
}
