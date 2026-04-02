use systemprompt_identifiers::{ScheduledJobId, JobName, DbValue, ToDbValue};

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
    let id = ScheduledJobId::new("job-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"job-1\"");
    let deserialized: ScheduledJobId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
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
fn job_name_from_str_and_string_equal() {
    let a: JobName = "x".into();
    let b: JobName = String::from("x").into();
    assert_eq!(a, b);
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
