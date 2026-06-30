<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-analytics

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-analytics.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-analytics.svg">
    <img alt="systemprompt-analytics terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-analytics.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-analytics.svg?style=flat-square)](https://crates.io/crates/systemprompt-analytics)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-analytics?style=flat-square)](https://docs.rs/systemprompt-analytics)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Analytics for systemprompt.io AI governance infrastructure. Session, agent, tool, and microdollar-precision cost attribution across the MCP governance pipeline. Comprehensive session tracking, behavioral bot detection, engagement metrics, funnel analytics, and anomaly detection.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)

This crate provides comprehensive analytics capabilities including:

- **Session Management** - Create, track, and manage user sessions with fingerprint-based identification
- **Behavioral Bot Detection** - Server-side detection of automated traffic using 7-signal analysis
- **Engagement Tracking** - Client-side engagement metrics (scroll depth, time on page, clicks)
- **Funnel Analytics** - Track user progression through defined conversion funnels
- **Anomaly Detection** - Real-time threshold-based and trend anomaly detection
- **Platform Statistics** - Aggregated metrics for users, agents, tools, costs, and traffic

## Usage

```toml
[dependencies]
systemprompt-analytics = "0.17.1"
```

Optional `geolocation` feature enables MaxMind GeoIP enrichment via `maxminddb`:

```toml
systemprompt-analytics = { version = "0.17.1", features = ["geolocation"] }
```

## Directory Structure

```
src/
├── lib.rs                          # Public exports and GeoIpReader type alias
├── error.rs                        # AnalyticsError enum (thiserror)
├── extension.rs                    # AnalyticsExtension schema registration
├── models/
│   ├── mod.rs                      # Core analytics models (sessions, stats, trends)
│   ├── engagement.rs               # Engagement event models
│   ├── events.rs                   # Analytics event types and payloads
│   ├── fingerprint.rs              # Fingerprint reputation models
│   ├── funnel.rs                   # Funnel tracking models
│   └── cli/
│       ├── mod.rs                  # CLI row-type re-exports
│       ├── agent.rs                # Agent CLI row types
│       ├── content.rs              # Content CLI row types
│       ├── overview.rs             # Overview CLI row types
│       ├── request.rs              # Request CLI row types
│       ├── session.rs              # Session CLI row types
│       └── tool.rs                 # Tool CLI row types
├── repository/
│   ├── mod.rs                      # Repository re-exports
│   ├── cli_sessions.rs             # CLI session statistics
│   ├── content_analytics.rs        # Content performance metrics
│   ├── conversations.rs            # Conversation analytics
│   ├── costs.rs                    # Cost breakdown queries
│   ├── engagement.rs               # Engagement event CRUD
│   ├── events.rs                   # Analytics event storage
│   ├── overview.rs                 # Dashboard overview metrics
│   ├── queries.rs                  # AI provider usage queries
│   ├── requests.rs                 # AI request analytics
│   ├── traffic.rs                  # Traffic source analysis
│   ├── agents/                     # Agent analytics (list, detail, stats)
│   ├── core_stats/                 # Platform stats (overview, activity, breakdowns, leaderboards)
│   ├── fingerprint/                # Fingerprint reputation (queries, mutations)
│   ├── funnel/                     # Funnel tracking (finders, mutations, stats, types)
│   ├── session/                    # Session lifecycle (queries, mutations, behavioral, types)
│   └── tools/                      # MCP tool execution analytics (list, detail)
└── services/
    ├── mod.rs                      # Service re-exports
    ├── ai_crawler_keywords.rs      # AI crawler user-agent patterns
    ├── ai_provider.rs              # AnalyticsAiSessionProvider
    ├── anomaly_detection.rs        # Threshold and trend anomaly detection
    ├── bot_keywords.rs             # matches_bot_pattern helper
    ├── detection.rs                # Detection constants
    ├── providers.rs                # Provider helpers
    ├── service.rs                  # AnalyticsService for session lifecycle
    ├── session_cleanup.rs          # Inactive session cleanup
    ├── throttle.rs                 # Progressive rate limiting
    ├── user_agent.rs               # User-agent parsing
    ├── behavioral_detector/        # 7-signal bot detection (checks, fingerprint_checks, helpers, types)
    └── extractor/                  # Request parsing and GeoIP enrichment

schema/
├── anomaly_thresholds.sql
├── engagement_events.sql
├── fingerprint_reputation.sql
├── funnels.sql
├── funnel_progress.sql
└── migrations/
    └── 003_seed_anomaly_thresholds.sql
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

## Throttle Levels

| Level | Rate Multiplier | Allows Requests |
|-------|-----------------|-----------------|
| Normal | 1.0x | Yes |
| Warning | 0.5x | Yes |
| Severe | 0.25x | Yes |
| Blocked | 0.0x | No |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-analytics)** · **[docs.rs](https://docs.rs/systemprompt-analytics)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
