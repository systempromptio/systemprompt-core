//! Tests for AnomalyDetectionService async operations

use systemprompt_analytics::{AnomalyDetectionService, AnomalyLevel};

mod anomaly_detection_service_tests {
    use super::*;

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
        let result = service.check_anomaly("requests_per_minute", 20.0).await;

        assert_eq!(result.level, AnomalyLevel::Warning);
        assert!(result.message.as_ref().expect("warning should have message").contains("WARNING"));
    }

    #[tokio::test]
    async fn check_anomaly_returns_critical_for_high_value() {
        let service = AnomalyDetectionService::new();
        let result = service.check_anomaly("requests_per_minute", 50.0).await;

        assert_eq!(result.level, AnomalyLevel::Critical);
        assert!(result.message.as_ref().expect("critical should have message").contains("CRITICAL"));
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

        let result = service.check_anomaly("session_count_per_fingerprint", 3.0).await;
        assert_eq!(result.level, AnomalyLevel::Normal);

        let result = service.check_anomaly("session_count_per_fingerprint", 7.0).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

        let result = service.check_anomaly("session_count_per_fingerprint", 15.0).await;
        assert_eq!(result.level, AnomalyLevel::Critical);
    }

    #[tokio::test]
    async fn check_anomaly_error_rate_thresholds() {
        let service = AnomalyDetectionService::new();

        let result = service.check_anomaly("error_rate", 0.05).await;
        assert_eq!(result.level, AnomalyLevel::Normal);

        let result = service.check_anomaly("error_rate", 0.15).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

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

        service.update_threshold("custom_metric", 5.0, 10.0).await;

        let result = service.check_anomaly("custom_metric", 7.0).await;
        assert_eq!(result.level, AnomalyLevel::Warning);

        let result = service.check_anomaly("custom_metric", 15.0).await;
        assert_eq!(result.level, AnomalyLevel::Critical);
    }

    #[tokio::test]
    async fn update_threshold_overwrites_existing() {
        let service = AnomalyDetectionService::new();

        service.update_threshold("requests_per_minute", 100.0, 200.0).await;

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

        for _ in 0..5 {
            service.record_event("spike_metric", 10.0).await;
        }

        service.record_event("spike_metric", 100.0).await;

        let result = service.check_trend_anomaly("spike_metric", 60).await;
        let result = result.expect("spike should be detected");
        assert!(
            result.level == AnomalyLevel::Critical || result.level == AnomalyLevel::Warning,
            "Expected Critical or Warning, got {:?}",
            result.level
        );
    }

    #[tokio::test]
    async fn check_trend_anomaly_detects_elevated() {
        let service = AnomalyDetectionService::new();

        for _ in 0..5 {
            service.record_event("elevated_metric", 10.0).await;
        }

        service.record_event("elevated_metric", 35.0).await;

        let result = service.check_trend_anomaly("elevated_metric", 60).await;
        let result = result.expect("elevated trend should be detected");
        assert_eq!(result.level, AnomalyLevel::Warning);
    }

    #[tokio::test]
    async fn check_trend_anomaly_returns_none_for_normal_trend() {
        let service = AnomalyDetectionService::new();

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

        let handle = tokio::spawn(async move {
            for i in 0..10 {
                service_clone.record_event("concurrent_metric", i as f64).await;
            }
        });

        for _ in 0..5 {
            let _ = service.check_anomaly("concurrent_metric", 5.0).await;
        }

        handle.await.unwrap();

        let events = service.get_recent_events("concurrent_metric").await;
        assert_eq!(events.len(), 10);
    }
}
