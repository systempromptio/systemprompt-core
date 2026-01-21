# systemprompt-analytics

Analytics domain crate for SystemPrompt providing session tracking, behavioral analysis, engagement metrics, bot detection, and platform statistics.

## Overview

This crate provides comprehensive analytics capabilities including:

- **Session Management** - Create, track, and manage user sessions with fingerprint-based identification
- **Behavioral Bot Detection** - Server-side detection of automated traffic using 7-signal analysis
- **Engagement Tracking** - Client-side engagement metrics (scroll depth, time on page, clicks)
- **Funnel Analytics** - Track user progression through defined conversion funnels
- **Anomaly Detection** - Real-time threshold-based and trend anomaly detection
- **Platform Statistics** - Aggregated metrics for users, agents, tools, costs, and traffic

## Directory Structure

```
src/
├── lib.rs                     # Public exports and GeoIpReader type alias
├── error.rs                   # AnalyticsError enum with thiserror
├── models/
│   ├── mod.rs                 # Core analytics models (sessions, stats, trends)
│   ├── cli.rs                 # CLI-specific row types for analytics queries
│   ├── engagement.rs          # Engagement event models
│   ├── events.rs              # Analytics event types and data structures
│   ├── fingerprint.rs         # Fingerprint reputation models
│   └── funnel.rs              # Funnel tracking models
├── repository/
│   ├── mod.rs                 # Repository re-exports
│   ├── agents.rs              # Agent task analytics queries
│   ├── cli_sessions.rs        # CLI session statistics
│   ├── content_analytics.rs   # Content performance metrics
│   ├── conversations.rs       # Conversation analytics
│   ├── core_stats.rs          # Platform-wide statistics
│   ├── costs.rs               # Cost breakdown queries
│   ├── engagement.rs          # Engagement event CRUD
│   ├── events.rs              # Analytics event storage
│   ├── fingerprint.rs         # Fingerprint reputation operations
│   ├── funnel.rs              # Funnel progress tracking
│   ├── overview.rs            # Dashboard overview metrics
│   ├── queries.rs             # AI provider usage queries
│   ├── requests.rs            # AI request analytics
│   ├── tools.rs               # MCP tool execution analytics
│   ├── traffic.rs             # Traffic source analysis
│   └── session/
│       ├── mod.rs             # SessionRepository facade
│       ├── mutations.rs       # Session create/update operations
│       ├── queries.rs         # Session read operations
│       └── types.rs           # Session parameter types
└── services/
    ├── mod.rs                 # Service re-exports
    ├── anomaly_detection.rs   # Real-time anomaly detection
    ├── behavioral_detector.rs # 7-signal bot detection
    ├── detection.rs           # Detection constants and helpers
    ├── extractor.rs           # Request parsing and bot detection
    ├── service.rs             # AnalyticsService for session lifecycle
    ├── session_cleanup.rs     # Inactive session cleanup
    └── throttle.rs            # Progressive rate limiting

schema/
├── anomaly_thresholds.sql
├── engagement_events.sql
├── fingerprint_reputation.sql
├── funnels.sql
├── funnel_progress.sql
└── migrations/
    ├── 001_engagement_not_null.sql
    ├── 002_add_content_id.sql
    ├── 003_backfill_content_id.sql
    └── 004_add_funnels.sql
```

## Key Components

### Services

| Service | Purpose |
|---------|---------|
| `AnalyticsService` | Session lifecycle management and analytics extraction |
| `AnomalyDetectionService` | Threshold-based and trend anomaly detection |
| `BehavioralBotDetector` | 7-signal server-side bot detection |
| `SessionCleanupService` | Cleanup of inactive sessions |
| `ThrottleService` | Progressive rate limiting (Normal/Warning/Severe/Blocked) |

### Repositories

| Repository | Purpose |
|------------|---------|
| `SessionRepository` | Session CRUD and behavioral data queries |
| `EngagementRepository` | Engagement event operations |
| `FingerprintRepository` | Fingerprint reputation tracking |
| `FunnelRepository` | Funnel progress and statistics |
| `AnalyticsEventsRepository` | Analytics event storage |
| `CoreStatsRepository` | Platform statistics and trends |
| `AgentAnalyticsRepository` | Agent task analytics |
| `ToolAnalyticsRepository` | MCP tool execution analytics |
| `RequestAnalyticsRepository` | AI request analytics |
| `CostAnalyticsRepository` | Cost breakdown queries |
| `TrafficAnalyticsRepository` | Traffic source analysis |
| `ContentAnalyticsRepository` | Content performance metrics |
| `OverviewAnalyticsRepository` | Dashboard metrics |
| `ConversationAnalyticsRepository` | Conversation statistics |
| `CliSessionAnalyticsRepository` | CLI session statistics |

### Models

| Model | Purpose |
|-------|---------|
| `AnalyticsSession` | Session data with tracking fields |
| `AnalyticsEvent` | Event with type, category, severity |
| `EngagementEvent` | Client-side engagement metrics |
| `FingerprintReputation` | Fingerprint tracking and flags |
| `Funnel`, `FunnelStep`, `FunnelProgress` | Funnel tracking |
| `ThrottleLevel`, `EscalationCriteria` | Rate limiting |
| `AnomalyCheckResult`, `AnomalyLevel` | Anomaly detection |
| `BehavioralAnalysisResult`, `BehavioralSignal` | Bot detection |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool access |
| `systemprompt-models` | Shared types including ContentRouting |
| `systemprompt-identifiers` | SessionId, UserId, FunnelId, etc. |
| `systemprompt-traits` | Repository trait |

## Behavioral Bot Detection

The `BehavioralBotDetector` analyzes sessions using 7 signals:

| Signal | Points | Threshold |
|--------|--------|-----------|
| High Request Count | 30 | > 50 requests |
| High Page Coverage | 25 | > 60% of site pages |
| Sequential Navigation | 20 | Systematic crawl pattern |
| Multiple Fingerprint Sessions | 20 | > 5 sessions per fingerprint |
| Regular Timing | 15 | < 0.1 timing variance |
| High Pages Per Minute | 15 | > 5 pages/min |
| Outdated Browser | 10 | Chrome < 90 or Firefox < 88 |

Sessions with score >= 50 are marked as behavioral bots.

## Throttle Levels

| Level | Rate Multiplier | Allows Requests |
|-------|-----------------|-----------------|
| Normal | 1.0x | Yes |
| Warning | 0.5x | Yes |
| Severe | 0.25x | Yes |
| Blocked | 0.0x | No |

## Exports

### From `lib.rs`

```rust
pub use error::{AnalyticsError, Result as AnalyticsResult};
pub use models::{...};  // 40+ model types
pub use repository::{...};  // 15 repositories + types
pub use services::{...};  // Services + detection helpers
pub type GeoIpReader = std::sync::Arc<maxminddb::Reader<Vec<u8>>>;
```
