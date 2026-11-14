# systemprompt-notifications-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-notifications-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-notifications-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-notifications-entry?style=flat-square)](https://docs.rs/systemprompt-notifications-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The notification HTTP routes of systemprompt.io: streams, rules, subscriptions, the feed and escalation.

**Layer**: Entry. The `http` feature carries the routes; the `cli` feature is reserved for the capability's commands. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Routes

| Path | Gate | Rate limit | Role group |
|------|------|------------|------------|
| `/api/v1/notifications` | user | `notifications` scope, notifications budget | content and misc |
| `/api/v1/admin/notifications` | admin | `notifications` scope, notifications budget | content and misc |
| `/v1/bridge/notifications*` | gateway credential, verified in the handler | `bridge_notifications` scope, notifications budget, bearer window | gateway, inside the gateway access log |

`NotificationsHttpExtension` declares the first two through `Extension::routers` and registers the bridge inbox into the gateway surface with `register_gateway_route!`, so the API server mounts all three without naming this crate. All three groups belong to the extension id `notifications-http`: disabling it in the profile unmounts every group (404). The extension requires the notification service slot that the `notifications` extension provides, so disabling `notifications` alone refuses boot; disable both to switch notifications off.
