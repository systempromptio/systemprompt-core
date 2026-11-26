# M1. Core server changes

**Goal.** The instance can push to a phone, knows its phones, lists a user's runs, pauses a run for a person, vouches for its apps, and tells a phone where the app is.

**Entry.** None for lanes B1, B2 and B4. Lane B3 needs the harness's remote-runtime-and-session-record stage (S4).

**Specification.** `../architecture/03-core-changes.md` (C1 to C5), `../architecture/07-push-and-approvals.md`.

## Wave A — contract, one lane

| Lane | Owned paths | Work |
|---|---|---|
| **M1-A1 Contract** | Route constants and wire types in the shared models crate for: device fields and push tokens; the runs list; the approval request, decision and events; the two association documents; the discovery document's `clients` section | Types and route constants only. No handler, no storage. Every later lane compiles against this |

**Wave gate (L1).** Offline check of core.

## Wave B — four lanes, parallel

| Lane | Owned paths | Work |
|---|---|---|
| **M1-B1 Push channel** | `crates/domain/notifications/src/channels/{apns,fcm}/**`; the notifications config plane | Two `NotificationChannel` implementations contributed through `NotificationsSlot`. Credentials as instance secrets. Payload of id, category and collapse key only. Suppression when a foreground stream is live. Token revocation on a provider's "unregistered" answer. No credentials configured means no channel contributed |
| **M1-B2 Device records and tokens** | `crates/domain/users/schema/migrations/` (one new migration); the device repository and service; `crates/entry/notifications/src/http/devices/**` | Platform, operating-system version, app version, app identifier, last seen on the device record. A push-token table owned by the device. Register, refresh and revoke routes for the caller's own device. Removal on sign-out, revocation and user purge; the purge inventory lists the table |
| **M1-B3 Sessions from a phone** | The notification written when a turn needs approval; list filters by state and mode; starting a `Remote` session with no workspace or a repository checkout; mode availability reported for the calling surface | Built on the harness's S4: the runtime, `Remote` mode, the session record and approvals already exist |
| **M1-B4 Association files and discovery** | `crates/entry/api/src/routes/wellknown/{apple,android}/**`; the bridge discovery builder; `crates/entry/gateway/src/http/bridge/release/**` | The two association documents from profile configuration; neither served when no app is configured. The discovery `clients` section. `android` as a download platform, with checksum. Per-platform minimum versions |

The migration in M1-B2 follows core's migration rules for large tables and is immutable once pushed.

**Wave gate (L1).** Offline check of core's root workspace; the source gates.

## Wave C — three reviews, parallel, read-only

| Lane | Reviews |
|---|---|
| **M1-C1** **(review)** | Tokens: a caller writes only its own device's token; a revoked device cannot re-register; purge removes tokens |
| **M1-C2** **(review)** | Approval: owner only; timeout denies; a second decision cannot overturn the first; a person cannot allow what policy denies; no host is silently run permissively |
| **M1-C3** **(review)** | Association files: served only from configuration; cannot be influenced by request input; cache headers correct |

## Stage gate (L2)

Core's push run: lints on all targets, the full test workspace, the schema upgrade ladder (one new migration), documentation, supply chain. New tests include: a notification delivered through a stub provider; an unregistered token revoked; an approval allowed, denied, timed out and decided twice; the association documents for a configured and an unconfigured instance.

## Exit criteria

1. Stage gate green.
2. No open high-rated review finding.
3. The web admin can list a user's runs and decide an approval through the new routes. That is the proof the server half works before any app exists.

## Decisions reserved to people

- The default approval timeout.
- Merging the programme branch into `future`.
