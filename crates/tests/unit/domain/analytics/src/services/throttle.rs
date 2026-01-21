//! Tests for throttle service types.

use chrono::{Duration, Utc};
use systemprompt_analytics::{EscalationCriteria, ThrottleLevel, ThrottleService};

mod throttle_level_tests {
    use super::*;

    #[test]
    fn normal_from_0() {
        assert_eq!(ThrottleLevel::from(0), ThrottleLevel::Normal);
    }

    #[test]
    fn warning_from_1() {
        assert_eq!(ThrottleLevel::from(1), ThrottleLevel::Warning);
    }

    #[test]
    fn severe_from_2() {
        assert_eq!(ThrottleLevel::from(2), ThrottleLevel::Severe);
    }

    #[test]
    fn blocked_from_3() {
        assert_eq!(ThrottleLevel::from(3), ThrottleLevel::Blocked);
    }

    #[test]
    fn default_to_normal_for_unknown_values() {
        assert_eq!(ThrottleLevel::from(-1), ThrottleLevel::Normal);
        assert_eq!(ThrottleLevel::from(4), ThrottleLevel::Normal);
        assert_eq!(ThrottleLevel::from(100), ThrottleLevel::Normal);
    }

    #[test]
    fn normal_to_i32() {
        let level: i32 = ThrottleLevel::Normal.into();
        assert_eq!(level, 0);
    }

    #[test]
    fn warning_to_i32() {
        let level: i32 = ThrottleLevel::Warning.into();
        assert_eq!(level, 1);
    }

    #[test]
    fn severe_to_i32() {
        let level: i32 = ThrottleLevel::Severe.into();
        assert_eq!(level, 2);
    }

    #[test]
    fn blocked_to_i32() {
        let level: i32 = ThrottleLevel::Blocked.into();
        assert_eq!(level, 3);
    }

    #[test]
    fn rate_multiplier_normal() {
        assert!((ThrottleLevel::Normal.rate_multiplier() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_multiplier_warning() {
        assert!((ThrottleLevel::Warning.rate_multiplier() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_multiplier_severe() {
        assert!((ThrottleLevel::Severe.rate_multiplier() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_multiplier_blocked() {
        assert!((ThrottleLevel::Blocked.rate_multiplier() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn allows_requests_normal() {
        assert!(ThrottleLevel::Normal.allows_requests());
    }

    #[test]
    fn allows_requests_warning() {
        assert!(ThrottleLevel::Warning.allows_requests());
    }

    #[test]
    fn allows_requests_severe() {
        assert!(ThrottleLevel::Severe.allows_requests());
    }

    #[test]
    fn allows_requests_blocked() {
        assert!(!ThrottleLevel::Blocked.allows_requests());
    }

    #[test]
    fn escalate_normal_to_warning() {
        assert_eq!(ThrottleLevel::Normal.escalate(), ThrottleLevel::Warning);
    }

    #[test]
    fn escalate_warning_to_severe() {
        assert_eq!(ThrottleLevel::Warning.escalate(), ThrottleLevel::Severe);
    }

    #[test]
    fn escalate_severe_to_blocked() {
        assert_eq!(ThrottleLevel::Severe.escalate(), ThrottleLevel::Blocked);
    }

    #[test]
    fn escalate_blocked_stays_blocked() {
        assert_eq!(ThrottleLevel::Blocked.escalate(), ThrottleLevel::Blocked);
    }

    #[test]
    fn deescalate_normal_stays_normal() {
        assert_eq!(ThrottleLevel::Normal.deescalate(), ThrottleLevel::Normal);
    }

    #[test]
    fn deescalate_warning_to_normal() {
        assert_eq!(ThrottleLevel::Warning.deescalate(), ThrottleLevel::Normal);
    }

    #[test]
    fn deescalate_severe_to_warning() {
        assert_eq!(ThrottleLevel::Severe.deescalate(), ThrottleLevel::Warning);
    }

    #[test]
    fn deescalate_blocked_to_severe() {
        assert_eq!(ThrottleLevel::Blocked.deescalate(), ThrottleLevel::Severe);
    }

    #[test]
    fn throttle_level_is_eq() {
        assert_eq!(ThrottleLevel::Normal, ThrottleLevel::Normal);
        assert_ne!(ThrottleLevel::Normal, ThrottleLevel::Warning);
    }

    #[test]
    fn throttle_level_is_copy() {
        let level = ThrottleLevel::Severe;
        let copied = level;
        assert_eq!(level, copied);
    }

    #[test]
    fn throttle_level_is_debug() {
        let debug_str = format!("{:?}", ThrottleLevel::Warning);
        assert!(debug_str.contains("Warning"));
    }

    #[test]
    fn throttle_level_serializes() {
        let level = ThrottleLevel::Severe;
        let json = serde_json::to_string(&level).unwrap();
        assert!(json.contains("Severe") || json.contains("2"));
    }

    #[test]
    fn throttle_level_deserializes() {
        let json = "\"Warning\"";
        let level: ThrottleLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level, ThrottleLevel::Warning);
    }
}

mod escalation_criteria_tests {
    use super::*;

    fn create_criteria(
        bot_score: i32,
        request_count: i64,
        error_rate: f64,
        rpm: f64,
    ) -> EscalationCriteria {
        EscalationCriteria {
            behavioral_bot_score: bot_score,
            request_count,
            error_rate,
            requests_per_minute: rpm,
        }
    }

    #[test]
    fn criteria_stores_values() {
        let criteria = create_criteria(50, 100, 0.1, 15.0);

        assert_eq!(criteria.behavioral_bot_score, 50);
        assert_eq!(criteria.request_count, 100);
        assert!((criteria.error_rate - 0.1).abs() < f64::EPSILON);
        assert!((criteria.requests_per_minute - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn criteria_is_copy() {
        let criteria = create_criteria(30, 50, 0.05, 10.0);
        let copied = criteria;

        assert_eq!(criteria.behavioral_bot_score, copied.behavioral_bot_score);
        assert_eq!(criteria.request_count, copied.request_count);
    }

    #[test]
    fn criteria_is_clone() {
        let criteria = create_criteria(30, 50, 0.05, 10.0);
        let cloned = criteria.clone();

        assert_eq!(criteria.behavioral_bot_score, cloned.behavioral_bot_score);
    }

    #[test]
    fn criteria_is_debug() {
        let criteria = create_criteria(50, 100, 0.2, 20.0);
        let debug_str = format!("{:?}", criteria);

        assert!(debug_str.contains("EscalationCriteria"));
        assert!(debug_str.contains("behavioral_bot_score"));
    }
}

mod throttle_service_tests {
    use super::*;

    fn create_criteria(
        bot_score: i32,
        request_count: i64,
        error_rate: f64,
        rpm: f64,
    ) -> EscalationCriteria {
        EscalationCriteria {
            behavioral_bot_score: bot_score,
            request_count,
            error_rate,
            requests_per_minute: rpm,
        }
    }

    #[test]
    fn new_creates_service() {
        let service = ThrottleService::new();
        let _ = format!("{:?}", service);
    }

    #[test]
    fn should_escalate_returns_false_when_blocked() {
        let criteria = create_criteria(100, 1000, 1.0, 100.0);
        assert!(!ThrottleService::should_escalate(&criteria, ThrottleLevel::Blocked));
    }

    #[test]
    fn should_escalate_on_high_bot_score() {
        let criteria = create_criteria(50, 10, 0.0, 5.0);
        assert!(ThrottleService::should_escalate(&criteria, ThrottleLevel::Normal));
    }

    #[test]
    fn should_escalate_on_high_rpm() {
        let criteria = create_criteria(0, 10, 0.0, 31.0);
        assert!(ThrottleService::should_escalate(&criteria, ThrottleLevel::Normal));
    }

    #[test]
    fn should_escalate_on_high_error_rate_with_requests() {
        let criteria = create_criteria(0, 25, 0.6, 5.0);
        assert!(ThrottleService::should_escalate(&criteria, ThrottleLevel::Normal));
    }

    #[test]
    fn should_not_escalate_on_high_error_rate_with_few_requests() {
        let criteria = create_criteria(0, 15, 0.6, 5.0);
        assert!(!ThrottleService::should_escalate(&criteria, ThrottleLevel::Normal));
    }

    #[test]
    fn should_not_escalate_on_normal_traffic() {
        let criteria = create_criteria(20, 10, 0.05, 10.0);
        assert!(!ThrottleService::should_escalate(&criteria, ThrottleLevel::Normal));
    }

    #[test]
    fn adjusted_rate_limit_normal() {
        let limit = ThrottleService::adjusted_rate_limit(100, ThrottleLevel::Normal);
        assert_eq!(limit, 100);
    }

    #[test]
    fn adjusted_rate_limit_warning() {
        let limit = ThrottleService::adjusted_rate_limit(100, ThrottleLevel::Warning);
        assert_eq!(limit, 50);
    }

    #[test]
    fn adjusted_rate_limit_severe() {
        let limit = ThrottleService::adjusted_rate_limit(100, ThrottleLevel::Severe);
        assert_eq!(limit, 25);
    }

    #[test]
    fn adjusted_rate_limit_blocked() {
        let limit = ThrottleService::adjusted_rate_limit(100, ThrottleLevel::Blocked);
        assert_eq!(limit, 1); // Minimum is 1
    }

    #[test]
    fn adjusted_rate_limit_minimum_is_one() {
        let limit = ThrottleService::adjusted_rate_limit(1, ThrottleLevel::Blocked);
        assert_eq!(limit, 1);
    }

    #[test]
    fn can_deescalate_returns_false_for_normal() {
        assert!(!ThrottleService::can_deescalate(ThrottleLevel::Normal, None, 30));
    }

    #[test]
    fn can_deescalate_returns_true_when_no_escalation_time() {
        assert!(ThrottleService::can_deescalate(ThrottleLevel::Warning, None, 30));
    }

    #[test]
    fn can_deescalate_returns_false_when_recently_escalated() {
        let recent = Utc::now() - Duration::minutes(10);
        assert!(!ThrottleService::can_deescalate(
            ThrottleLevel::Warning,
            Some(recent),
            30
        ));
    }

    #[test]
    fn can_deescalate_returns_true_after_cooldown() {
        let past = Utc::now() - Duration::minutes(60);
        assert!(ThrottleService::can_deescalate(
            ThrottleLevel::Warning,
            Some(past),
            30
        ));
    }

    #[test]
    fn can_deescalate_respects_cooldown_minutes() {
        // Well before cooldown - should not allow deescalate
        let before_cooldown = Utc::now() - Duration::minutes(15);
        assert!(!ThrottleService::can_deescalate(
            ThrottleLevel::Warning,
            Some(before_cooldown),
            30
        ));

        // Well after cooldown - should allow deescalate
        let after_cooldown = Utc::now() - Duration::minutes(60);
        assert!(ThrottleService::can_deescalate(
            ThrottleLevel::Warning,
            Some(after_cooldown),
            30
        ));
    }

    #[test]
    fn throttle_service_is_default() {
        let service = ThrottleService::default();
        let _ = format!("{:?}", service);
    }

    #[test]
    fn throttle_service_is_copy() {
        let service = ThrottleService::new();
        let copied = service;
        let _ = format!("{:?}", copied);
    }
}
