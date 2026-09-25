# systemprompt-analytics

[![Crates.io](https://img.shields.io/crates/v/systemprompt-analytics.svg?style=flat-square)](https://crates.io/crates/systemprompt-analytics)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-analytics?style=flat-square)](https://docs.rs/systemprompt-analytics)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Queries session, request, tool and cost data through analytics-owned views over its owners' source tables, for usage analysis and operational reporting.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)

This crate provides:

- **Session Analysis** - Extract request signals and analyze sessions through users-owned persistence contracts
- **Behavioral Bot Detection** - Server-side detection of automated traffic using 7-signal analysis
- **Engagement Tracking** - Client-side engagement metrics (scroll depth, time on page, clicks)
- **Platform Statistics** - Aggregated metrics for users, agents, tools, costs, and traffic

## Usage

```toml
[dependencies]
systemprompt-analytics = "0.62"
```

Optional `geolocation` feature enables MaxMind GeoIP enrichment via `maxminddb`:

```toml
systemprompt-analytics = { version = "0.62", features = ["geolocation"] }
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | Analytics models: sessions, events, engagement, fingerprints, plus CLI row types. |
| `repository/` | Reporting queries over the `report_*` views, local engagement/fingerprint persistence, and injected owner interfaces. |
| `services/` | `AnalyticsService` request extraction, behavioral detection, session-cleanup orchestration, and request/GeoIP enrichment. |

Schema DDL lives in `schema/*.sql` (`engagement_events`, `fingerprint_reputation`, and the `report_*` views) with migrations in `schema/migrations/`. Migration `015_retire_feedback_and_reporting_projection` drops the tables of the retired feedback engine, reporting projection, funnels and anomaly thresholds.

## Key Components

### Services

| Service | Purpose |
|---------|---------|
| `AnalyticsService` | Request analytics extraction |
| `BehavioralBotDetector` | 7-signal server-side bot detection |

### Repositories

| Repository | Purpose |
|------------|---------|
| `SessionRepository` | Delegates session operations to users and behavioral event/content reads to their owners |
| `EngagementRepository` | Engagement event operations |
| `FingerprintRepository` | Fingerprint reputation tracking |
| `AnalyticsEventsRepository` | Logging-owned event ingestion |
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
| `EngagementEvent` | Client-side engagement metrics |
| `FingerprintReputation` | Fingerprint tracking and flags |
| `BehavioralAnalysisResult`, `BehavioralSignal` | Bot detection |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool access |
| `systemprompt-extension` | Extension trait and schema registration |
| `systemprompt-models` | Shared types including `ContentRouting` |
| `systemprompt-identifiers` | `SessionId`, `UserId`, and other typed IDs |
| `systemprompt-traits` | Injected session, event-store and content-count contracts |
| `maxminddb` (optional) | GeoIP database reader behind `geolocation` feature |

## Behavioral Bot Detection

Behavioral decisions read the authoritative owner stores. Cross-domain reports
read the `report_*` views this crate declares (`schema/report_views.sql`), one
per source table: nothing is copied, so a report is current as of its query,
and the views hide every row of a user whose status is `deleted`. Runtime
composition supplies the users session store, logging event store and content
catalog statistics; analytics does not depend on those owner crates.

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

