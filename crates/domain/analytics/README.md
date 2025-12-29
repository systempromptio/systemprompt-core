# systemprompt-core-analytics

Session tracking, behavioral analysis, engagement metrics, and ML feature preparation for SystemPrompt.

## Directory Structure

```
src/
├── lib.rs                          # Public exports and GeoIpReader type alias
├── models/
│   ├── mod.rs                      # Core analytics models (sessions, stats, trends)
│   ├── engagement.rs               # Client-side engagement event models
│   ├── fingerprint.rs              # Fingerprint reputation models
│   └── ml_features.rs              # ML behavioral feature models
├── repository/
│   ├── mod.rs                      # Repository re-exports
│   ├── core_stats.rs               # Platform stats, activity trends, top users/agents
│   ├── engagement.rs               # Engagement event CRUD operations
│   ├── fingerprint.rs              # Fingerprint reputation operations
│   ├── ml_features.rs              # ML feature storage and retrieval
│   ├── queries.rs                  # AI provider usage queries
│   └── session.rs                  # Session CRUD and behavioral data
└── services/
    ├── mod.rs                      # Service re-exports
    ├── anomaly_detection.rs        # Real-time anomaly detection with thresholds
    ├── behavioral_detector.rs      # Server-side bot detection signals
    ├── extractor.rs                # Request parsing, bot detection, geo lookup
    ├── feature_extraction.rs       # ML feature computation from session data
    ├── service.rs                  # AnalyticsService for session lifecycle
    └── throttle.rs                 # Progressive throttling based on behavior
schema/
├── anomaly_thresholds.sql          # Configurable anomaly thresholds
├── engagement_events.sql           # Client-side engagement tracking
├── fingerprint_reputation.sql      # Fingerprint reputation tracking
└── ml_behavioral_features.sql      # ML training data storage
```

## Key Files

| File | Purpose |
|------|---------|
| `models/mod.rs` | Core data structures for sessions, events, stats |
| `models/engagement.rs` | EngagementEvent, CreateEngagementEventInput |
| `models/ml_features.rs` | MlBehavioralFeatures, FeatureExtractionConfig |
| `repository/session.rs` | Session repository with behavioral data queries |
| `repository/engagement.rs` | Engagement event CRUD operations |
| `repository/ml_features.rs` | ML feature storage and labeled data retrieval |
| `services/anomaly_detection.rs` | Threshold-based and trend anomaly detection |
| `services/behavioral_detector.rs` | 7-signal bot detection (request velocity, timing, etc.) |
| `services/feature_extraction.rs` | ML feature computation from session + engagement data |
| `services/throttle.rs` | Progressive rate limiting (Normal/Warning/Severe/Blocked) |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-core-database` | Database pool |
| `systemprompt-models` | Shared types |
| `systemprompt-identifiers` | SessionId, UserId |
| `systemprompt-traits` | Repository traits |

## Exports

### Services
- `AnalyticsService` - Session lifecycle management
- `AnomalyDetectionService` - Real-time anomaly detection
- `FeatureExtractionService` - ML feature computation
- `BehavioralBotDetector` - Server-side bot detection
- `ThrottleService` - Progressive throttling

### Repositories
- `SessionRepository` - Session database operations
- `EngagementRepository` - Engagement event operations
- `MlFeaturesRepository` - ML feature storage
- `FingerprintRepository` - Fingerprint reputation
- `CoreStatsRepository` - Platform statistics
- `AnalyticsQueryRepository` - AI provider usage

### Models
- `AnalyticsSession`, `AnalyticsEvent` - Core session types
- `EngagementEvent`, `CreateEngagementEventInput` - Engagement tracking
- `MlBehavioralFeatures`, `FeatureExtractionConfig` - ML features
- `FingerprintReputation`, `FlagReason` - Fingerprint tracking
- `AnomalyThreshold`, `AnomalyLevel`, `AnomalyCheckResult` - Anomaly detection
- `ThrottleLevel`, `EscalationCriteria` - Throttling
- 20+ additional stat/trend models
