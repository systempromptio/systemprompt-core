# Analytics System Architecture

This document describes the analytics system in systemprompt-core, including session tracking, event recording, real-time streaming, funnel tracking, and reporting.

---

## Overview

The analytics system tracks user behavior across the platform with four main components:

1. **Session Tracking** - Automatic HTTP request tracking via middleware
2. **Unified Events API** - Generic event recording for all event types
3. **Real-time Streaming** - SSE endpoint for live analytics dashboards
4. **Funnel Tracking** - Conversion funnel definition and progress tracking

---

## API Endpoints

### Unified Events API

The new unified events API replaces the legacy engagement endpoint with a flexible, generic event system.

**Base Path:** `/api/v1/analytics`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/events` | POST | Record a single event |
| `/events/batch` | POST | Record multiple events |
| `/stream` | GET | Real-time SSE stream |

---

### Record Single Event

**Endpoint:** `POST /api/v1/analytics/events`

Records a single analytics event. Session is automatically attached from the request context.

**Request:**
```json
{
  "event_type": "page_view",
  "page_url": "/blog/my-post",
  "slug": "my-post",
  "content_id": "uuid (optional, auto-resolved from slug)",
  "referrer": "https://google.com",
  "data": {
    "custom_field": "custom_value"
  }
}
```

**Event Types:**

| Type | Category | Description |
|------|----------|-------------|
| `page_view` | navigation | Page load event |
| `page_exit` | navigation | Page unload with engagement data |
| `link_click` | interaction | Click on a link |
| `scroll` | engagement | Scroll milestone reached |
| `engagement` | engagement | Detailed engagement snapshot |
| `conversion` | conversion | Goal/conversion completed |
| `custom_string` | custom | Any custom event type |

**Response:** `201 Created`
```json
{
  "id": "evt_abc123",
  "event_type": "page_view"
}
```

---

### Record Batch Events

**Endpoint:** `POST /api/v1/analytics/events/batch`

Records multiple events in a single request. Useful for submitting queued events.

**Request:**
```json
{
  "events": [
    {
      "event_type": "page_view",
      "page_url": "/blog/post-1"
    },
    {
      "event_type": "link_click",
      "page_url": "/blog/post-1",
      "data": {
        "target_url": "/blog/post-2",
        "link_text": "Read more"
      }
    },
    {
      "event_type": "page_exit",
      "page_url": "/blog/post-1",
      "data": {
        "scroll_depth": 85,
        "time_on_page_ms": 45000,
        "click_count": 12
      }
    }
  ]
}
```

**Response:** `201 Created`
```json
{
  "recorded": 3,
  "events": [
    { "id": "evt_abc123", "event_type": "page_view" },
    { "id": "evt_abc124", "event_type": "link_click" },
    { "id": "evt_abc125", "event_type": "page_exit" }
  ]
}
```

---

### Real-time Analytics Stream

**Endpoint:** `GET /api/v1/analytics/stream`

Server-Sent Events (SSE) stream for real-time analytics. Connect to receive live events.

**Event Types in Stream:**

| Event | Description |
|-------|-------------|
| `SESSION_STARTED` | New session detected |
| `SESSION_ENDED` | Session closed with duration/stats |
| `PAGE_VIEW` | Page view recorded |
| `ENGAGEMENT_UPDATE` | Engagement metrics update |
| `REAL_TIME_STATS` | Periodic aggregated statistics |
| `HEARTBEAT` | Keep-alive heartbeat |

**Example Stream Events:**
```
data: {"type":"HEARTBEAT","timestamp":"2024-01-15T10:00:00Z"}

data: {"type":"SESSION_STARTED","timestamp":"2024-01-15T10:00:01Z","session_id":"sess_123","device_type":"desktop","browser":"Chrome","is_bot":false}

data: {"type":"PAGE_VIEW","timestamp":"2024-01-15T10:00:02Z","session_id":"sess_123","page_url":"/blog/hello","content_id":"cont_456"}

data: {"type":"REAL_TIME_STATS","timestamp":"2024-01-15T10:00:05Z","active_sessions":42,"active_users":38,"requests_per_minute":156,"page_views_last_5m":89,"bot_requests_last_5m":12}
```

---

## Event Data Schemas

### Engagement Data

For `page_exit` or `engagement` event types:

```json
{
  "scroll_depth": 85,
  "time_on_page_ms": 45000,
  "time_to_first_interaction_ms": 1500,
  "time_to_first_scroll_ms": 3000,
  "click_count": 12,
  "mouse_move_distance_px": 15000,
  "keyboard_events": 0,
  "copy_events": 2,
  "visible_time_ms": 40000,
  "hidden_time_ms": 5000,
  "is_rage_click": false,
  "is_dead_click": false,
  "reading_pattern": "engaged"
}
```

### Link Click Data

For `link_click` event type:

```json
{
  "target_url": "/blog/other-post",
  "link_text": "Read more",
  "link_position": "article-body",
  "is_external": false
}
```

### Scroll Data

For `scroll` event type:

```json
{
  "depth": 75,
  "milestone": 75,
  "direction": "down",
  "velocity": 2.5
}
```

### Conversion Data

For `conversion` event type:

```json
{
  "goal_name": "newsletter_signup",
  "goal_value": 10.0,
  "funnel_step": 3
}
```

---

## Funnel Tracking

Define conversion funnels to track user journeys.

### Funnel Definition

Funnels are defined with ordered steps that match URL patterns or event types:

```sql
-- Example funnel: Blog to Newsletter
INSERT INTO funnels (id, name, description) VALUES
  ('funnel_blog_newsletter', 'Blog to Newsletter', 'Track blog readers who subscribe');

INSERT INTO funnel_steps (funnel_id, step_order, name, match_pattern, match_type) VALUES
  ('funnel_blog_newsletter', 0, 'Visit Blog', '/blog/', 'url_prefix'),
  ('funnel_blog_newsletter', 1, 'Read Article', '/blog/[^/]+', 'url_regex'),
  ('funnel_blog_newsletter', 2, 'Click Subscribe', 'link_click', 'event_type'),
  ('funnel_blog_newsletter', 3, 'Complete Signup', '/newsletter/confirmed', 'url_exact');
```

### Match Types

| Type | Description | Example |
|------|-------------|---------|
| `url_exact` | Exact URL match | `/blog/hello-world` |
| `url_prefix` | URL starts with pattern | `/blog/` |
| `url_regex` | Regex pattern match | `/blog/[^/]+` |
| `event_type` | Match event type | `link_click` |

### Funnel Progress

Progress is automatically tracked per session:

| Field | Description |
|-------|-------------|
| `funnel_id` | Funnel being tracked |
| `session_id` | Session in funnel |
| `current_step` | Highest step reached |
| `step_timestamps` | JSON array of step completion times |
| `completed_at` | Set when all steps completed |
| `dropped_at_step` | Step where user dropped off |

---

## Session Tracking (Automatic)

Session tracking happens automatically via the `AnalyticsMiddleware`. No client-side code required.

### Tracked Data

| Field | Description |
|-------|-------------|
| `session_id` | Unique session identifier |
| `user_id` | Authenticated user ID (if logged in) |
| `ip_address` | Client IP (for geo-lookup) |
| `user_agent` | Browser user agent string |
| `device_type` | desktop, mobile, tablet |
| `browser` | Chrome, Firefox, Safari, etc. |
| `os` | Windows, macOS, Linux, iOS, Android |
| `country` | GeoIP-resolved country |
| `referrer_source` | Traffic source classification |
| `utm_*` | UTM tracking parameters |
| `is_bot` | Known bot detection flag |
| `is_behavioral_bot` | Behavioral analysis bot flag |

---

## Bot Detection

The system uses two detection methods:

### 1. Known Pattern Detection

Detects bots based on:
- User agent strings (Googlebot, Bingbot, etc.)
- Request paths (/.env, /wp-admin, /admin.php)
- Malicious scanner patterns

### 2. Behavioral Detection (7-Signal System)

| Signal | Description | Threshold |
|--------|-------------|-----------|
| Request Velocity | Requests per minute | >60/min |
| Page Coverage | Unique pages per session | >50 pages |
| Time Between Requests | Consistency analysis | <100ms avg |
| Session Duration | Abnormally long sessions | >4 hours |
| Click Patterns | Human-like click behavior | Analysis |
| Scroll Patterns | Natural scrolling behavior | Analysis |
| Mouse Movement | Human-like mouse patterns | Analysis |

---

## CLI Commands

### Analytics Overview

```bash
sp analytics overview
sp --json analytics overview --since 7d
```

### Content Analytics

```bash
sp analytics content stats
sp analytics content top
sp analytics content trends
```

### Traffic Analysis

```bash
sp analytics traffic sources
sp analytics traffic geo
sp analytics traffic devices
sp analytics traffic bots
```

### Session Analytics

```bash
sp analytics sessions stats
sp analytics sessions live
sp analytics sessions trends --since 7d
```

---

## Database Schema

### Core Tables

| Table | Purpose | Location |
|-------|---------|----------|
| `user_sessions` | Session tracking | `domain/users/schema/` |
| `analytics_events` | All event logging | `infra/logging/schema/` |
| `engagement_events` | Legacy engagement (deprecated) | `domain/analytics/schema/` |
| `funnels` | Funnel definitions | `domain/analytics/schema/` |
| `funnel_steps` | Funnel step definitions | `domain/analytics/schema/` |
| `funnel_progress` | Session funnel progress | `domain/analytics/schema/` |
| `fingerprint_reputation` | Fingerprint tracking | `domain/analytics/schema/` |

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Clients                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐  │
│  │ Browser/JS      │  │ Mobile App      │  │ Server (redirect)       │  │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘  │
└───────────┼─────────────────────┼───────────────────────┼────────────────┘
            │                     │                       │
            ▼                     ▼                       ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Unified Events API                               │
│                                                                          │
│  POST /api/v1/analytics/events                                          │
│  POST /api/v1/analytics/events/batch                                    │
│  GET  /api/v1/analytics/stream  (SSE)                                   │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Event Types: page_view | page_exit | link_click | scroll |      │  │
│  │               engagement | conversion | custom                    │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
            ┌────────────────────────┼────────────────────────┐
            ▼                        ▼                        ▼
┌────────────────────┐  ┌────────────────────┐  ┌────────────────────────┐
│  analytics_events  │  │ engagement_events  │  │  funnel_progress       │
│  (all events)      │  │ (legacy metrics)   │  │  (journey tracking)    │
└────────────────────┘  └────────────────────┘  └────────────────────────┘
            │                        │                        │
            └────────────────────────┼────────────────────────┘
                                     │
                                     ▼
                    ┌────────────────────────────────┐
                    │      Analytics Broadcaster      │
                    │   (Real-time SSE to dashboard)  │
                    └────────────────────────────────┘
```

---

## Integration Guide

### Client-Side Tracking

```javascript
const ANALYTICS_ENDPOINT = '/api/v1/analytics/events';

async function trackEvent(eventType, pageUrl, data = {}) {
  await fetch(ANALYTICS_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include',
    body: JSON.stringify({
      event_type: eventType,
      page_url: pageUrl,
      data: data
    })
  });
}

trackEvent('page_view', '/blog/hello-world');

trackEvent('link_click', '/blog/hello-world', {
  target_url: '/blog/another-post',
  link_text: 'Read more'
});

trackEvent('page_exit', '/blog/hello-world', {
  scroll_depth: 85,
  time_on_page_ms: 45000,
  click_count: 12
});
```

### Batch Submission

```javascript
const eventQueue = [];

function queueEvent(eventType, pageUrl, data = {}) {
  eventQueue.push({ event_type: eventType, page_url: pageUrl, data });
}

async function flushEvents() {
  if (eventQueue.length === 0) return;

  const events = [...eventQueue];
  eventQueue.length = 0;

  await fetch('/api/v1/analytics/events/batch', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include',
    body: JSON.stringify({ events })
  });
}

window.addEventListener('pagehide', flushEvents);
setInterval(flushEvents, 30000);
```

### Real-time Dashboard

```javascript
const eventSource = new EventSource('/api/v1/analytics/stream');

eventSource.onmessage = (event) => {
  const data = JSON.parse(event.data);

  switch (data.type) {
    case 'REAL_TIME_STATS':
      updateDashboard(data);
      break;
    case 'PAGE_VIEW':
      addToLiveList(data);
      break;
    case 'SESSION_STARTED':
      incrementActiveUsers();
      break;
  }
};

eventSource.onerror = () => {
  setTimeout(() => location.reload(), 5000);
};
```

---

## Files Reference

| Component | Path |
|-----------|------|
| Analytics domain | `crates/domain/analytics/` |
| Events models | `crates/domain/analytics/src/models/events.rs` |
| Events repository | `crates/domain/analytics/src/repository/events.rs` |
| Funnel models | `crates/domain/analytics/src/models/funnel.rs` |
| Funnel repository | `crates/domain/analytics/src/repository/funnel.rs` |
| Analytics routes | `crates/entry/api/src/routes/analytics/` |
| Events handlers | `crates/entry/api/src/routes/analytics/events.rs` |
| SSE stream handler | `crates/entry/api/src/routes/analytics/stream.rs` |
| Stream event types | `crates/shared/models/src/events/analytics_event.rs` |
| Analytics broadcaster | `crates/infra/events/src/services/routing.rs` |
| Session schema | `crates/domain/users/schema/user_sessions.sql` |
| Funnel schema | `crates/domain/analytics/schema/migrations/004_add_funnels.sql` |
| CLI commands | `crates/entry/cli/src/commands/analytics/` |

---

## Migration from Legacy Engagement Endpoint

The `/api/v1/engagement` endpoint is deprecated. Migrate to the new unified events API:

### Before (Legacy)

```javascript
fetch('/api/v1/engagement', {
  method: 'POST',
  body: JSON.stringify({
    page_url: '/blog/post',
    time_on_page_ms: 45000,
    max_scroll_depth: 85,
    click_count: 12
  })
});
```

### After (New API)

```javascript
fetch('/api/v1/analytics/events', {
  method: 'POST',
  body: JSON.stringify({
    event_type: 'page_exit',
    page_url: '/blog/post',
    data: {
      time_on_page_ms: 45000,
      scroll_depth: 85,
      click_count: 12
    }
  })
});
```
