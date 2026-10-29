use systemprompt_manifest::profile::{ConsumerProfile, NotificationsProfile};

#[test]
fn notifications_profile_defaults_bound_every_table_and_loop() {
    let profile = NotificationsProfile::default();
    assert_eq!(profile.retention_days, 90);
    assert_eq!(profile.max_audience, 500);
    assert_eq!(profile.unread_max_age_days, 180);
    assert_eq!(profile.failure_retention_days, 30);
    let consumer = profile.consumer;
    assert_eq!(consumer.batch, 64);
    assert_eq!(consumer.poll_secs, 5);
    assert_eq!(consumer.sweep_secs, 30);
    assert_eq!(consumer.resolver_timeout_ms, 5000);
    assert_eq!(consumer.channel_timeout_ms, 5000);
}

#[test]
fn a_partial_notifications_block_keeps_the_other_defaults() {
    let profile: NotificationsProfile =
        serde_yaml::from_str("max_audience: 50\nconsumer:\n  poll_secs: 1\n").expect("parses");
    assert_eq!(profile.max_audience, 50);
    assert_eq!(profile.retention_days, 90);
    assert_eq!(profile.consumer.poll_secs, 1);
    assert_eq!(profile.consumer.batch, ConsumerProfile::default().batch);
}
