# 03. Changes to core

Five server changes, two extensions of device crates, one new crate. All land on `future`.

## 1. Summary

| # | Change | Crate | New crate | Touches authorization | Stage |
|---|---|---|---|---|---|
| C1 | Push channel | `domain/notifications` | No | No (delivery only) | M1 |
| C2 | Device records and push tokens | `domain/users`, `entry/notifications` | No | Yes (who may register a token for whom) | M1 |
| C3 | Runs list and the approval step | `app/agent-runs`, `entry/agent-runs` | No | Yes (who may decide) | M1 |
| C4 | Association files | `entry/api` well-known routes; profile configuration | No | Yes (which apps the domain vouches for) | M1 |
| C5 | Discovery and downloads for phones | `shared/models` bridge instance types; `entry/gateway` release routes | No | No | M1 |
| C6 | Typed client calls | `systemprompt-client` | No | No | M2 |
| C7 | Phone platform modules | `systemprompt-device` | No | No | M2 |
| C8 | The binding layer | `systemprompt-mobile` | **Yes** | No | M2 |
| C9 | Closure gate for the phone composition | Gate scripts | No | No | M0 |

`systemprompt-mobile` is the ninth new crate of the two programmes. It has to be its own crate because it is built as a static and a dynamic library and carries the binding generator's scaffolding; no existing crate can take that build shape.

No new host. A phone makes no model call and receives no host profile; it is identified by the existing iOS and Android client ids.

## 2. C1 — Push channel

- Two implementations of the existing `NotificationChannel` trait: one for Apple's push service, one for Google's. Each is contributed through `NotificationsSlot`, so the pipeline's leases, receipts and retries apply unchanged.
- A channel delivers only when the recipient has a live token for that platform and the notification's attention level warrants a push; otherwise the in-app channel alone delivers.
- Credentials are secrets of the instance, declared in a config plane: a signing key, key id and team id for Apple; a service account for Google. An instance with no credentials configured contributes no push channel and nothing else changes.
- The payload carries an opaque notification id, a category and a collapse key. No title, no body, no run content. The app fetches what to show (`07-push-and-approvals.md`).
- A token the provider rejects as unregistered is revoked in the same delivery.

## 3. C2 — Device records and push tokens

- The device record gains: platform, operating-system version, app version, app identifier, last seen. A new migration; existing rows default to "unknown".
- A push token belongs to one device record. Routes under the notifications surface: register or refresh the calling device's token; revoke it. A caller can only write its own device's token.
- Signing out, revoking a device, or deleting a user removes its tokens. The user-purge inventory lists the new table.
- The admin device list shows platform and app version.

## 4. C3 — Runs list and the approval step

**List.** `GET` on the runs collection returns the caller's own runs, newest first, paged, filterable by state.

**Approval.** A run can pause for a person.

| Element | Design |
|---|---|
| When | A run is queued with a permission mode that asks. When the agent requests a tool the mode does not pre-allow, the adapter raises a permission request instead of deciding locally |
| State | The run moves to `awaiting_approval`; an event of kind `approval_requested` is stored with the tool, a redacted summary of its input and a request id |
| Notify | A notification with high attention is written in the same transaction, so push and the in-app stream both carry it |
| Decide | `POST` to the run's approvals with the request id and `allow` or `deny`, optionally with feedback for the agent. Only the run's owner may decide. The first decision wins; a second receives the recorded outcome |
| Resume | The adapter returns the decision to the agent; an `approval_decided` event is stored; the run continues or the tool is refused |
| Timeout | A request with no decision after a configured interval is **denied**, never allowed |
| Audit | Request, decision, decider, client and time are in the run's events and the governance journal |

Which hosts can pause: those whose headless mode supports a permission hook. For the native engine (harness S7) the pause is native. A host that cannot pause is refused at queue time when an asking mode is requested; it is never silently run in a permissive mode.

## 5. C4 — Association files

- `/.well-known/apple-app-site-association` and `/.well-known/assetlinks.json`, built from profile configuration: for each app the instance vouches for, its Apple team and bundle identifier, or its Android package name and signing-certificate fingerprints.
- They enable passkeys in the native apps and verified links that open the app.
- An instance with no apps configured serves neither file.
- They are static per configuration, unauthenticated, cacheable.

## 6. C5 — Discovery and downloads

- The discovery document gains a `clients` section: per platform, the app identifier, the minimum supported version, and where to get it. For Android, a download path on the instance; for iOS, the organisation's private-distribution or beta link.
- The gateway's release proxy accepts `android` as a platform and serves the signed APK with its checksum, from the install's own release source.
- The version window gains per-platform minimums for the two phone platforms.

## 7. C6 — Typed client calls

Added to `systemprompt-client`: agents and contexts; runs (queue, list, get, cancel, events, outputs); approvals; the three event streams with resume; notifications (feed, unread, read, token registration); documents (list, read, comments). Typed against the same route constants the server uses. The desktop shell uses them too.

## 8. C7 — Phone platform modules

`systemprompt-device` gains iOS and Android implementations of its platform contract: credential sealing in the platform keystore, owner-only file storage in the app's container, and base directories. No scheduler, elevation, machine policy or single-instance module applies to phones.

## 9. C8 — The binding layer

Specified in `04-shared-core-and-bindings.md`.

## 10. What does not change

- No change to the gateway's model routes, the governor, or host identity.
- No WebSocket. Server-sent events with resume are sufficient for a foreground app; push covers the background.
- No server component is added outside existing crates.
