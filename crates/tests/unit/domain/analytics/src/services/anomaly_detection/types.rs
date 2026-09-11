//! Tests for anomaly detection types: AnomalyThresholdConfig, AnomalyEvent,
//! AnomalyLevel, AnomalyCheckResult

use systemprompt_analytics::{
    AnomalyCheckResult, AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig,
};

mod anomaly_threshold_config_tests {
    use super::*;

    #[test]
    fn config_stores_thresholds() {
        let config = AnomalyThresholdConfig {
            warning_threshold: 10.0,
            critical_threshold: 25.0,
        };

        assert!((config.warning_threshold - 10.0).abs() < f64::EPSILON);
        assert!((config.critical_threshold - 25.0).abs() < f64::EPSILON);
    }
}

mod anomaly_event_tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn event_stores_timestamp_and_value() {
        let now = Utc::now();
        let event = AnomalyEvent {
            timestamp: now,
            value: 42.5,
        };

        assert_eq!(event.timestamp, now);
        assert!((event.value - 42.5).abs() < f64::EPSILON);
    }
}

mod anomaly_level_tests {
    use super::*;

    #[test]
    fn levels_are_different() {
        assert_ne!(AnomalyLevel::Normal, AnomalyLevel::Warning);
        assert_ne!(AnomalyLevel::Warning, AnomalyLevel::Critical);
        assert_ne!(AnomalyLevel::Normal, AnomalyLevel::Critical);
    }
}

mod anomaly_check_result_tests {
    use super::*;

    fn create_result(metric: &str, value: f64, level: AnomalyLevel) -> AnomalyCheckResult {
        AnomalyCheckResult {
            metric_name: metric.to_string(),
            current_value: value,
            level,
            message: if level != AnomalyLevel::Normal {
                Some(format!("{} anomaly at {}", metric, value))
            } else {
                None
            },
        }
    }

    #[test]
    fn result_stores_metric_name() {
        let result = create_result("requests_per_minute", 15.0, AnomalyLevel::Warning);
        assert_eq!(result.metric_name, "requests_per_minute");
    }

    #[test]
    fn result_stores_current_value() {
        let result = create_result("error_rate", 0.25, AnomalyLevel::Critical);
        assert!((result.current_value - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn result_stores_level() {
        let result = create_result("metric", 10.0, AnomalyLevel::Warning);
        assert_eq!(result.level, AnomalyLevel::Warning);
    }

    #[test]
    fn result_normal_has_no_message() {
        let result = create_result("metric", 5.0, AnomalyLevel::Normal);
        assert!(result.message.is_none());
    }

    #[test]
    fn result_warning_has_message() {
        let result = create_result("metric", 15.0, AnomalyLevel::Warning);
        result
            .message
            .as_ref()
            .expect("warning should have message");
    }

    #[test]
    fn result_critical_has_message() {
        let result = create_result("metric", 30.0, AnomalyLevel::Critical);
        result
            .message
            .as_ref()
            .expect("critical should have message");
    }
}
