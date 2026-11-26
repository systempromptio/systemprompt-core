# 01. Current state

Read on core `future` at `2b7e8dd3d`, 2026-11-26. Nothing was built or run.

## 1. What core already offers a phone

| Need | Exists today | Where |
|---|---|---|
| Live events | Server-sent event streams for contexts, agent UI events and agent-to-agent events; a per-run event stream that resumes from a cursor. No WebSocket | `crates/entry/agent/src/http/stream/`, `crates/entry/agent-runs/src/http/`, `crates/infra/events/src/sse.rs` |
| Start and follow a run | Queue a run with an idempotency key; get it; cancel it; page or stream its events; read its outputs. A run is visible only to the user who queued it | `crates/entry/agent-runs/src/http/mod.rs`, `crates/app/agent-runs/` |
| Notifications feed | A durable outbox, a consumer, recipient planning with rules, quiet hours and coalescing, delivery under a lease with receipts and retries; feed, unread, read, subscriptions and rules routes | `crates/domain/notifications/`, `crates/entry/notifications/src/http/mod.rs` |
| A place to add a delivery channel | The `NotificationChannel` trait, contributed through `NotificationsSlot`. One channel ships: in-app delivery on the recipient's event stream | `crates/shared/extension/src/notifications/traits.rs`, `crates/domain/notifications/src/contract.rs` |
| Sign-in for a native app | Authorization code with PKCE; refresh tokens; dynamic client registration; `native` clients may register private-use custom-scheme redirects; passkey registration and sign-in routes | `crates/domain/oauth/`, `crates/entry/identity/src/http/oauth/` |
| Linking a second device | A device-link exchange: a code redeemed for a durable device credential, and for a person-facing client a PKCE-bound authorization code | `crates/entry/identity/src/http/bridge/device_link.rs` |
| Client identity | Client ids for iOS and Android already exist beside web, command line, desktop and bridge | `crates/shared/identifiers/src/client.rs` |
| Documents | List, read, content, comments, revisions, presence, export | `crates/entry/files/src/http/documents/` |
| Serving client downloads | The gateway resolves the newest client release and proxies its bytes per platform, with caching and checksums | `crates/entry/gateway/src/http/bridge/release/` |
| Version floor | A minimum and maximum client version window that clients learn through their heartbeat | `crates/shared/models/src/bridge/instance.rs` |
| Shared crates that compile for phones | `identifiers`, `wire`, `provider-contracts`, `traits`, `models`, `client`: no native TLS, no process or signal APIs, no SQL parser; the database driver is optional everywhere. The HTTP client is built without a cryptography provider, so one must be installed at start-up | The crates' manifests |

## 2. What is missing

| Missing | Consequence for a phone |
|---|---|
| Any push channel, and anywhere to register a device's push token | No alert reaches a phone unless the app is open |
| Platform, operating-system version and app version on device records; the record holds only an id, a user, a fingerprint, a label and two timestamps | The instance cannot target, list or retire phones |
| A route listing a user's runs | The phone cannot show "my runs" |
| A server-side approval step. Runs are headless: a permission mode is chosen up front and nothing pauses for a person | There is nothing to approve from a phone |
| `apple-app-site-association` and `assetlinks.json` | Passkeys and verified app links cannot work in a native app |
| Mobile entries, download locations and minimum versions in the discovery document | A phone cannot learn where to get the app or that it is too old |
| Enrolment that does not assume a desktop install id | A phone has no enrolment path of its own beyond the link exchange |
| A window toolkit that runs on phones. The desktop toolkit's maintainers closed both phone requests as not planned in July 2026; a third-party crate exists and is early | The desktop shell cannot be reused; the phone screens are native |

Not missing, though it might look so: a device authorization grant. Authorization code with PKCE and the device-link exchange cover a phone.

## 3. What the harness programme provides that this builds on

| From the harness stages | Used here for |
|---|---|
| `systemprompt-device` in core (harness S1; bridge code in S5) | Sign-in, key trust, sealed credentials, sync; this set adds its iOS and Android platform modules |
| `systemprompt-harness` composition and slices (harness S4) | The phone is one more composition: the `device` slice and a client slice, nothing else |
| `BrandDecl` and `ThemeDecl` as data (harness S4) | One brand and theme source for desktop and phone |
| The scripted instance (harness S0 and S2) | Every phone test |
| Server-owned sessions (harness S3) | The session list, the per-turn stream, the waiting states and approvals that every client, the phone included, renders |
| The template's `extensions/` layout, flavours and patch mechanism | `extensions/mobile/` and a `mobile` flavour |
