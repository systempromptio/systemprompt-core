# systemprompt-analytics-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-analytics-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-analytics-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-analytics-entry?style=flat-square)](https://docs.rs/systemprompt-analytics-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The analytics HTTP routes and CLI reports of systemprompt.io: event ingestion, live streams, and the traffic, session, cost and content reports.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, contributed to the CLI through `Contributions::command`. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Routes (`http`)

Declared by the `analytics-http` extension and mounted by the API server in the content-and-misc role group under the `content` rate-limit budget:

| Path | Gate | Purpose |
|------|------|---------|
| `POST /api/v1/analytics/events`, `POST /api/v1/analytics/events/batch` | admin | Record client analytics events; a `page_exit` event with time on page is also recorded as engagement. |
| `GET /api/v1/analytics/stream` | admin | Live analytics event stream (server-sent events). |
| `POST /track/engagement`, `POST /track/engagement/batch` | public | Record engagement events and mark the session converted on a conversion event. |

The live stream subscribes to the analytics event broadcaster, which the API hands to the `/api/v1/analytics` router alone; no other extension can read it from the shared extension state. A profile that disables `analytics-http` serves none of them (404). The `analytics` extension, which owns request recording and the protective jobs, is required and cannot be disabled.

## Commands (`cli`)

The top-level `analytics` group (`overview`, `conversations`, `agents`, `tools`, `requests`, `sessions`, `content`, `traffic`, `costs`), contributed by `AnalyticsHttpExtension` with the `LocalExport` and `SkipValidation` grants. With the `content-commands` feature as well, `core content popular`, a leaf under the `core content` group of `systemprompt-content-entry`; enable it only in a build whose composition also contributes that group (the `systemprompt` facade does so when both `analytics` and `content` are on). Every command is read-only and needs a profile (a run given only `--database-url` is refused); `analytics` writes `--export` files locally.
