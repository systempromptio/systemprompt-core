use chrono::{Duration, TimeZone, Utc};
use systemprompt_database::RoleCount;
use systemprompt_identifiers::{EnterpriseLicenseId, InstanceId, TenantId};
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::LicenceConfig;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_scheduler::jobs::licence_checkin::{
    JOB_NAME, LicenceCheckin, LicenceCheckinJob, checkin_due, sign_body,
};
use systemprompt_traits::Job;

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).unwrap();
    serde_yaml::from_str(&yaml).unwrap()
}

#[test]
fn signature_is_hmac_sha256_hex_over_the_body() {
    let signature = sign_body(b"Jefe", b"what do ya want for nothing?").unwrap();
    assert_eq!(
        signature,
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn signature_changes_with_the_body() {
    let a = sign_body(b"secret", br#"{"a":1}"#).unwrap();
    let b = sign_body(b"secret", br#"{"a":2}"#).unwrap();
    assert_ne!(a, b);
}

#[test]
fn licence_block_is_disabled_by_default() {
    let config: LicenceConfig = serde_yaml::from_str("{}").unwrap();
    assert!(!config.enabled());
    assert_eq!(config, LicenceConfig::default());
    assert_eq!(config.endpoint, LicenceConfig::DEFAULT_ENDPOINT);
    assert_eq!(config.interval_secs, LicenceConfig::DEFAULT_INTERVAL_SECS);

    let profile = fixture_profile();
    assert!(!profile.licence.enabled());
}

#[test]
fn job_is_not_scheduled_without_multi_tenant() {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let profile = systemprompt_config::LoadedProfile::from_path(&boot.profile_path).unwrap();
    let deps = Dependencies::new().with(profile);
    assert_eq!(LicenceCheckinJob.name(), JOB_NAME);
    assert!(!LicenceCheckinJob.configured(&deps));
}

#[test]
fn interval_below_the_floor_is_raised_to_it() {
    let config = LicenceConfig {
        multi_tenant: true,
        interval_secs: 60,
        ..LicenceConfig::default()
    };
    assert_eq!(
        config.effective_interval_secs(),
        LicenceConfig::MIN_INTERVAL_SECS
    );

    let now = Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap();
    assert!(checkin_due(&config, None, now));
    assert!(!checkin_due(
        &config,
        Some(now - Duration::seconds(120)),
        now
    ));
    assert!(checkin_due(
        &config,
        Some(now - Duration::seconds(3_600)),
        now
    ));
}

#[test]
fn interval_below_the_floor_fails_profile_validation() {
    let mut profile = fixture_profile();
    profile.licence.multi_tenant = true;
    profile.licence.interval_secs = 60;
    let error = profile.validate().unwrap_err().to_string();
    assert!(error.contains("licence.interval_secs"), "{error}");
}

#[test]
fn payload_carries_tenant_provider_host_and_role_counts() {
    let mut profile = fixture_profile();
    profile.server.external_url = "https://tenant.example.com/base".to_owned();
    profile.licence.licence_id = Some(EnterpriseLicenseId::new("lic_123"));
    let mut cloud = profile.cloud.clone().unwrap_or_default();
    cloud.tenant_id = Some(TenantId::new("tenant_42"));
    profile.cloud = Some(cloud);
    let roles = vec![
        RoleCount {
            role: "api".to_owned(),
            instances: 3,
        },
        RoleCount {
            role: "worker".to_owned(),
            instances: 1,
        },
    ];
    let at = Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap();

    let checkin = LicenceCheckin::build(&profile, InstanceId::new("node-a"), &roles, at);

    assert_eq!(
        checkin.licence_id.as_ref().map(|l| l.as_str()),
        Some("lic_123")
    );
    assert_eq!(
        checkin.tenant_id.as_ref().map(|t| t.as_str()),
        Some("tenant_42")
    );
    assert_eq!(checkin.instance_id.as_str(), "node-a");
    assert_eq!(checkin.external_host.as_deref(), Some("tenant.example.com"));
    assert_eq!(
        checkin.hosting_provider,
        profile.hosting_provider().as_str()
    );
    assert_eq!(
        checkin.core_version,
        systemprompt_extension::Extension::metadata(&systemprompt_scheduler::SchedulerExtension)
            .version
    );
    assert_eq!(checkin.instances_by_role.get("api"), Some(&3));
    assert_eq!(checkin.instances_by_role.get("worker"), Some(&1));

    let body = serde_json::to_value(&checkin).unwrap();
    assert_eq!(body["tenant_id"], "tenant_42");
    assert_eq!(body["instances_by_role"]["api"], 3);
    assert_eq!(body["timestamp"], "2026-10-28T12:00:00Z");
}
