use systemprompt_manifest::SecurityConfig;
use systemprompt_manifest::profile::UiSessionConfig;

use crate::profile_validation::{errors_of, valid_profile};

const MINIMAL_SECURITY: &str = "jwt_access_token_expiration: 3600\njwt_refresh_token_expiration: \
                                86400\njwt_audiences: [api]\n";

#[test]
fn the_defaults_are_five_minutes_twelve_hours_and_ten_minutes() {
    let defaults = UiSessionConfig::default();
    assert_eq!(defaults.access_ttl_secs, 300);
    assert_eq!(defaults.absolute_secs, 43_200);
    assert_eq!(defaults.step_up_max_age_secs, 600);
    assert!(defaults.is_default());
}

#[test]
fn a_profile_without_the_block_takes_the_defaults_and_validates() {
    let security: SecurityConfig = serde_yaml::from_str(MINIMAL_SECURITY).unwrap();
    assert_eq!(security.ui_session, UiSessionConfig::default());
    assert!(valid_profile().validate().is_ok());
}

#[test]
fn a_stated_field_keeps_the_defaults_of_the_others() {
    let yaml = format!("{MINIMAL_SECURITY}ui_session:\n  absolute_secs: 86400\n");
    let security: SecurityConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(
        security.ui_session,
        UiSessionConfig {
            absolute_secs: 86_400,
            ..UiSessionConfig::default()
        }
    );
    assert!(!security.ui_session.is_default());
}

#[test]
fn an_unknown_field_is_refused() {
    let yaml = format!("{MINIMAL_SECURITY}ui_session:\n  sliding_secs: 60\n");
    let err = serde_yaml::from_str::<SecurityConfig>(&yaml).expect_err("unknown field");
    assert!(err.to_string().contains("sliding_secs"), "{err}");
}

#[test]
fn the_default_block_is_left_out_of_a_written_profile() {
    let mut security: SecurityConfig = serde_yaml::from_str(MINIMAL_SECURITY).unwrap();
    assert!(
        !serde_yaml::to_string(&security)
            .unwrap()
            .contains("ui_session")
    );
    security.ui_session.access_ttl_secs = 120;
    let written = serde_yaml::to_string(&security).unwrap();
    assert!(written.contains("access_ttl_secs: 120"), "{written}");
}

#[test]
fn step_up_is_required_once_the_sign_in_is_older_than_the_window() {
    let now = 1_900_000_000;
    let defaults = UiSessionConfig::default();
    assert!(!defaults.step_up_required(now - 600, now));
    assert!(defaults.step_up_required(now - 601, now));

    let short = UiSessionConfig {
        step_up_max_age_secs: 120,
        ..defaults
    };
    assert!(!short.step_up_required(now - 120, now));
    assert!(short.step_up_required(now - 121, now));
}

fn errors_with(ui_session: UiSessionConfig) -> String {
    let mut profile = valid_profile();
    profile.security.ui_session = ui_session;
    errors_of(&profile)
}

#[test]
fn the_access_lifetime_is_bounded_to_one_minute_and_one_hour() {
    for (value, accepted) in [(59, false), (60, true), (3600, true), (3601, false)] {
        let errors = errors_with(UiSessionConfig {
            access_ttl_secs: value,
            ..UiSessionConfig::default()
        });
        assert_eq!(
            !errors.contains("security.ui_session.access_ttl_secs must be between 60 and 3600"),
            accepted,
            "{value}: {errors}"
        );
    }
}

#[test]
fn the_session_is_bounded_to_five_minutes_and_thirty_days() {
    for (value, accepted) in [
        (299, false),
        (300, true),
        (2_592_000, true),
        (2_592_001, false),
    ] {
        let errors = errors_with(UiSessionConfig {
            access_ttl_secs: 60,
            absolute_secs: value,
            step_up_max_age_secs: 60,
        });
        assert_eq!(
            !errors.contains("security.ui_session.absolute_secs must be between 300 and 2592000"),
            accepted,
            "{value}: {errors}"
        );
    }
}

#[test]
fn the_step_up_window_is_bounded_to_one_minute_and_the_session() {
    for (value, accepted) in [(59, false), (60, true), (43_200, true), (43_201, false)] {
        let errors = errors_with(UiSessionConfig {
            step_up_max_age_secs: value,
            ..UiSessionConfig::default()
        });
        assert_eq!(
            !errors.contains("security.ui_session.step_up_max_age_secs must be between 60 and"),
            accepted,
            "{value}: {errors}"
        );
    }
}

#[test]
fn an_access_lifetime_longer_than_the_session_is_refused() {
    let errors = errors_with(UiSessionConfig {
        access_ttl_secs: 900,
        absolute_secs: 600,
        step_up_max_age_secs: 600,
    });
    assert!(
        errors.contains("security.ui_session.access_ttl_secs (900) must not exceed"),
        "{errors}"
    );
    assert!(
        errors_with(UiSessionConfig {
            access_ttl_secs: 600,
            absolute_secs: 600,
            step_up_max_age_secs: 600,
        })
        .is_empty()
    );
}
