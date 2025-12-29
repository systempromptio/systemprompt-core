# systemprompt-analytics Unit Tests

## Crate Overview
Analytics, anomaly detection, behavioral bot detection, throttling, and session management. Provides traffic analysis and ML-based abuse detection.

## Source Files
- `src/services/anomaly_detection.rs` - AnomalyDetectionService
- `src/services/behavioral_detector.rs` - BehavioralBotDetector
- `src/services/extractor.rs` - SessionAnalytics
- `src/services/feature_extraction.rs` - FeatureExtractionService
- `src/services/service.rs` - AnalyticsService
- `src/services/session_cleanup.rs` - SessionCleanupService
- `src/services/throttle.rs` - ThrottleService
- `src/repository/session/` - SessionRepository

## Test Plan

### Anomaly Detection Tests
**Source:** `src/services/anomaly_detection.rs`
- `test_anomaly_check_normal_traffic` - Normal traffic passes
- `test_anomaly_check_high_velocity` - High velocity detection
- `test_anomaly_check_threshold_config` - Custom thresholds
- `test_anomaly_event_recording` - Event recording
- `test_anomaly_level_classification` - Level classification

### Behavioral Bot Detector Tests
**Source:** `src/services/behavioral_detector.rs`
- `test_behavioral_analyze_human_pattern` - Human patterns
- `test_behavioral_analyze_bot_pattern` - Bot patterns
- `test_behavioral_signal_types` - Signal type handling
- `test_behavioral_threshold_checking` - Threshold checks
- `test_behavioral_classification` - Classification output

### Throttle Service Tests
**Source:** `src/services/throttle.rs`
- `test_throttle_rate_limiting` - Rate limit enforcement
- `test_throttle_escalation` - Escalation criteria
- `test_throttle_level_determination` - Level determination
- `test_throttle_cooldown` - Cooldown behavior

### Feature Extraction Tests
**Source:** `src/services/feature_extraction.rs`
- `test_feature_extraction_behavioral` - Behavioral features
- `test_feature_extraction_ml_ready` - ML-ready format

### Session Management Tests
**Source:** `src/services/service.rs`
- `test_analytics_session_create` - Create session
- `test_analytics_session_update` - Update session
- `test_analytics_session_cleanup` - Cleanup old sessions
- `test_analytics_event_recording` - Record events

### Session Cleanup Tests
- `test_session_cleanup_expired` - Cleanup expired
- `test_session_cleanup_batch` - Batch cleanup

## Mocking Requirements
- Mock database
- Mock clock for time-based tests

## Test Fixtures Needed
- Sample session data
- Sample behavioral patterns
- Threshold configurations
