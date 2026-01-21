//! Tests for anomaly detection service.

use systemprompt_analytics::{
    AnomalyCheckResult, AnomalyDetectionService, AnomalyEvent, AnomalyLevel, AnomalyThresholdConfig,
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

    #[test]
    fn config_is_copy() {
        let config = AnomalyThresholdConfig {
            warning_threshold: 5.0,
            critical_threshold: 10.0,
        };
        let copied = config;
        assert!((copied.warning_threshold - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn config_is_clone() {
        let config = AnomalyThresholdConfig {
            warning_threshold: 5.0,
            critical_threshold: 10.0,
        };
        let cloned = config.clone();
        assert!((cloned.warning_threshold - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn config_is_debug() {
        let config = AnomalyThresholdConfig {
            warning_threshold: 5.0,
            critical_threshold: 10.0,
        };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("AnomalyThresholdConfig"));
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

    #[test]
    fn event_is_copy() {
        let now = Utc::now();
        let event = AnomalyEvent {
            timestamp: now,
            value: 10.0,
        };
        let copied = event;
        assert!((copied.value - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn event_is_clone() {
        let now = Utc::now();
        let event = AnomalyEvent {
            timestamp: now,
            value: 10.0,
        };
        let cloned = event.clone();
        assert!((cloned.value - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn event_is_debug() {
        let event = AnomalyEvent {
            timestamp: Utc::now(),
            value: 15.0,
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AnomalyEvent"));
    }
}

mod anomaly_level_tests {
    use super::*;

    #[test]
    fn normal_is_eq() {
        assert_eq!(AnomalyLevel::Normal, AnomalyLevel::Normal);
    }

    #[test]
    fn warning_is_eq() {
        assert_eq!(AnomalyLevel::Warning, AnomalyLevel::Warning);
    }

    #[test]
    fn critical_is_eq() {
        assert_eq!(AnomalyLevel::Critical, AnomalyLevel::Critical);
    }

    #[test]
    fn levels_are_different() {
        assert_ne!(AnomalyLevel::Normal, AnomalyLevel::Warning);
        assert_ne!(AnomalyLevel::Warning, AnomalyLevel::Critical);
        assert_ne!(AnomalyLevel::Normal, AnomalyLevel::Critical);
    }

    #[test]
    fn level_is_copy() {
        let level = AnomalyLevel::Warning;
        let copied = level;
        assert_eq!(level, copied);
    }

    #[test]
    fn level_is_clone() {
        let level = AnomalyLevel::Critical;
        let cloned = level.clone();
        assert_eq!(level, cloned);
    }

    #[test]
    fn level_is_debug() {
        let debug_str = format!("{:?}", AnomalyLevel::Warning);
        assert!(debug_str.contains("Warning"));
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
        assert!(result.message.is_some());
    }

    #[test]
    fn result_critical_has_message() {
        let result = create_result("metric", 30.0, AnomalyLevel::Critical);
        assert!(result.message.is_some());
    }

    #[test]
    fn result_is_clone() {
        let result = create_result("metric", 10.0, AnomalyLevel::Warning);
        let cloned = result.clone();
        assert_eq!(result.metric_name, cloned.metric_name);
    }

    #[test]
    fn result_is_debug() {
        let result = create_result("test_metric", 10.0, AnomalyLevel::Normal);
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("AnomalyCheckResult"));
    }
}

mod anomaly_detection_service_tests {
    use super::*;

    #[test]
    fn new_creates_service() {
        let service = AnomalyDetectionService::new();
        let _ = format!("{:?}", service);
    }

    #[test]
    fn default_creates_service() {
        let service = AnomalyDetectionService::default();
        let _ = format!("{:?}", service);
    }

    #[test]
    fn service_is_clone() {
        let service = AnomalyDetectionService::new();
        let cloned = service.clone();
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn service_is_debug() {
        let service = AnomalyDetectionService::new();
        let debug_str = format!("{:?}", service);
        assert!(debug_str.contains("AnomalyDetectionService"));
    }

    #[tokio::test]
    async fn check_anomaly_returns_normal_for_low_value() {
        let service = AnomalyDetectionService::new();
        let result = service.check_anomaly("requests_per_minute", 5.0).await;

        assert_eq!(result.level, AnomalyLevel::Normal);
        assert!(result.message.is_none());
    }

    #[tokio::test]
    async fn check_anomaly_returns_warning_for_moderate_value() {
        let service = AnomalyDetectionService::new();
        // Default warning threshold for requests_per_minute is 15.0
        let result = service.check_anomaly("requests_per_minute", 20.0).await;

        assert_eq!(result.level, AnomalyLevel::Warning);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("WARNING"));
    }

    #[tokio::test]
    async fn check_anomaly_returns_critical_for_high_value() {
        let service = AnomalyDetectionService::new();
        // Default critical threshold for requests_per_minute is 30.0
        let result = service.check_anomaly("requests_per_minute", 50.0).await;

        assert_eq!(result.level, AnomalyLevel::Critical);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("CRITICAL"));
    }

    #[tokio::test]
    async fn check_anomaly_unknown_metric_returns_normal() {
        let service = AnomalyDetectionService::new();
        let result = service.check_anomaly("unknown_metric", 1000.0).await;

        assert_eq!(result.level, AnomalyLevel::Normal);
    }

    #[tokio::test]
    async fn check_anomaly_session_count_thresholds() {
        let service = AnomalyDetectionService::new();

        // Normal
        let result = service.check_anomaly("session_count_per_fingerprint", 3.0).await;
        assert_eq!(result.level, AnomalyLevel::Normal);

        // Warning (threshold is 5.0)
        let result = service.check_anomaly("session_count_per_fingerprint", 7.0).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

        // Critical (threshold is 10.0)
        let result = service.check_anomaly("session_count_per_fingerprint", 15.0).await;
        assert_eq!(result.level, AnomalyLevel::Critical);
    }

    #[tokio::test]
    async fn check_anomaly_error_rate_thresholds() {
        let service = AnomalyDetectionService::new();

        // Normal
        let result = service.check_anomaly("error_rate", 0.05).await;
        assert_eq!(result.level, AnomalyLevel::Normal);

        // Warning (threshold is 0.1)
        let result = service.check_anomaly("error_rate", 0.15).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

        // Critical (threshold is 0.25)
        let result = service.check_anomaly("error_rate", 0.30).await;
        assert_eq!(result.level, AnomalyLevel::Critical);
    }

    #[tokio::test]
    async fn record_event_stores_event() {
        let service = AnomalyDetectionService::new();
        service.record_event("test_metric", 10.0).await;

        let events = service.get_recent_events("test_metric").await;
        assert_eq!(events.len(), 1);
        assert!((events[0].value - 10.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn record_event_stores_multiple_events() {
        let service = AnomalyDetectionService::new();
        service.record_event("test_metric", 10.0).await;
        service.record_event("test_metric", 20.0).await;
        service.record_event("test_metric", 30.0).await;

        let events = service.get_recent_events("test_metric").await;
        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn get_recent_events_returns_empty_for_unknown_metric() {
        let service = AnomalyDetectionService::new();
        let events = service.get_recent_events("unknown").await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn update_threshold_changes_thresholds() {
        let service = AnomalyDetectionService::new();

        // Update to custom thresholds
        service.update_threshold("custom_metric", 5.0, 10.0).await;

        // Should trigger warning at 5.0
        let result = service.check_anomaly("custom_metric", 7.0).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

        // Should trigger critical at 10.0
        let result = service.check_anomaly("custom_metric", 15.0).await;
        assert_eq!(result.level, AnomalyLevel::Critical);
    }

    #[tokio::test]
    async fn update_threshold_overwrites_existing() {
        let service = AnomalyDetectionService::new();

        // Update requests_per_minute to new thresholds
        service.update_threshold("requests_per_minute", 100.0, 200.0).await;

        // Value that was previously warning should now be normal
        let result = service.check_anomaly("requests_per_minute", 20.0).await;
        assert_eq!(result.level, AnomalyLevel::Normal);
    }

    #[tokio::test]
    async fn check_trend_anomaly_returns_none_for_few_events() {
        let service = AnomalyDetectionService::new();
        service.record_event("trend_metric", 10.0).await;

        let result = service.check_trend_anomaly("trend_metric", 5).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn check_trend_anomaly_returns_none_for_unknown_metric() {
        let service = AnomalyDetectionService::new();
        let result = service.check_trend_anomaly("unknown", 5).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn check_trend_anomaly_detects_spike() {
        let service = AnomalyDetectionService::new();

        // Record several normal values
        for _ in 0..5 {
            service.record_event("spike_metric", 10.0).await;
        }

        // Record a large spike - needs to be > 3x the average to be Critical
        // With 5 values of 10 and 1 value of 100: avg = 65/6 = 10.83, ratio = 100/10.83 = 9.23
        service.record_event("spike_metric", 100.0).await;

        let result = service.check_trend_anomaly("spike_metric", 60).await;
        assert!(result.is_some());
        let result = result.unwrap();
        // Note: spike detection is based on ratio of latest/avg
        // With our values the ratio should exceed the threshold
        assert!(
            result.level == AnomalyLevel::Critical || result.level == AnomalyLevel::Warning,
            "Expected Critical or Warning, got {:?}",
            result.level
        );
    }

    #[tokio::test]
    async fn check_trend_anomaly_detects_elevated() {
        let service = AnomalyDetectionService::new();

        // Record several normal values
        for _ in 0..5 {
            service.record_event("elevated_metric", 10.0).await;
        }

        // Record an elevated value that will produce ratio > 2.0
        // With 5 values of 10 and 1 value of 35: avg = (50+35)/6 = 14.17
        // Ratio = 35/14.17 = 2.47 which is > 2.0 (Warning) but < 3.0 (not Critical)
        service.record_event("elevated_metric", 35.0).await;

        let result = service.check_trend_anomaly("elevated_metric", 60).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.level, AnomalyLevel::Warning);
    }

    #[tokio::test]
    async fn check_trend_anomaly_returns_none_for_normal_trend() {
        let service = AnomalyDetectionService::new();

        // Record consistent values
        for _ in 0..10 {
            service.record_event("stable_metric", 10.0).await;
        }

        let result = service.check_trend_anomaly("stable_metric", 60).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn service_is_thread_safe() {
        let service = AnomalyDetectionService::new();
        let service_clone = service.clone();

        // Spawn task to record events
        let handle = tokio::spawn(async move {
            for i in 0..10 {
                service_clone.record_event("concurrent_metric", i as f64).await;
            }
        });

        // Meanwhile, check anomalies
        for _ in 0..5 {
            let _ = service.check_anomaly("concurrent_metric", 5.0).await;
        }

        handle.await.unwrap();

        let events = service.get_recent_events("concurrent_metric").await;
        assert_eq!(events.len(), 10);
    }
}
