# systemprompt-analytics

[![Crates.io](https://img.shields.io/crates/v/systemprompt-analytics.svg?style=flat-square)](https://crates.io/crates/systemprompt-analytics)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-analytics?style=flat-square)](https://docs.rs/systemprompt-analytics)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Queries session, request, tool and cost records for usage analysis and operational reporting.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)

This crate provides:

- **Session Management** - Create, track, and manage user sessions with fingerprint-based identification
- **Behavioral Bot Detection** - Server-side detection of automated traffic using 7-signal analysis
- **Engagement Tracking** - Client-side engagement metrics (scroll depth, time on page, clicks)
- **Funnel Analytics** - Track user progression through defined conversion funnels
- **Anomaly Detection** - Real-time threshold-based and trend anomaly detection
- **Platform Statistics** - Aggregated metrics for users, agents, tools, costs, and traffic

## Usage

```toml
[dependencies]
systemprompt-analytics = "0.51"
```

Optional `geolocation` feature enables MaxMind GeoIP enrichment via `maxminddb`:

```toml
systemprompt-analytics = { version = "0.51", features = ["geolocation"] }
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | Analytics models: sessions, events, engagement, fingerprints, funnels, plus CLI row types. |
| `repository/` | Compile-time-verified queries for sessions, agents, tools, requests, costs, traffic, content, funnels, fingerprints, and aggregate stats. |
| `services/` | `AnalyticsService` session lifecycle, `AnomalyDetectionService`, the `behavioral_detector/` 7-signal bot detection, session cleanup, and the request/GeoIP `extractor/`. |

Schema DDL lives in `schema/*.sql` (`anomaly_thresholds`, `engagement_events`, `fingerprint_reputation`, `funnels`, `funnel_progress`) with migrations in `schema/migrations/`:

- `001_add_engagement_event_type.sql`
- `002_add_engagement_event_data.sql`
- `003_seed_anomaly_thresholds.sql`

## Key Components

### Services

| Service | Purpose |
|---------|---------|
| `AnalyticsService` | Session lifecycle management and analytics extraction |
| `AnomalyDetectionService` | Threshold-based and trend anomaly detection |
| `BehavioralBotDetector` | 7-signal server-side bot detection |
| `SessionCleanupService` | Cleanup of inactive sessions |

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
| `AnomalyCheckResult`, `AnomalyLevel` | Anomaly detection |
| `BehavioralAnalysisResult`, `BehavioralSignal` | Bot detection |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool access |
| `systemprompt-extension` | Extension trait and schema registration |
| `systemprompt-models` | Shared types including `ContentRouting` |
| `systemprompt-identifiers` | `SessionId`, `UserId`, `FunnelId`, and other typed IDs |
| `systemprompt-traits` | Repository trait |
| `maxminddb` (optional) | GeoIP database reader behind `geolocation` feature |

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

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
