# Changelog

## [0.81.0] - 2026-11-12

### Breaking

- `NotificationRepository::{lock_pending, mark_delivered}` are replaced by `claim_pending`, `load_claimed` and `finish_claim` under a per-row lease. Migrate by claiming rows with a lease duration.

### Fixed

- Delivery holds no transaction open while sending.

## [0.78.0] - 2026-11-04

### Added

- `consumer::MAX_ROUND_TRANSIENTS`.

### Changed

- A consumer drain round commits once. It claims up to one profile batch of facts on one transaction, processes each in its own savepoint and acknowledges it on that transaction. A transient failure holds the fact back for the poll interval and the round goes on, ending early past `MAX_ROUND_TRANSIENTS`. The `pending` rows of the whole round are announced after its commit.

## [0.77.0] - 2026-11-01

### Breaking

- **Breaking:** **Notifications:** `actor_display` and `excerpt` are removed from notifications; a notification carries `actor_user_id`, `title` and `body` (migration 004). Migrate feed clients by resolving the actor's name from the users API and rendering `title`/`body`.
- **Breaking:** **Notifications:** `PipelineParts::actors` and `NotificationActorDirectory` are removed, and `NewRecipient::excerpt` is replaced by `title` and `body`. Migrate struct literals by dropping the field and setting both.
- **Breaking:** **Notifications:** `via_agent` is an `AgentName` on `NewNotifications` and `Notification`. Migrate by constructing it with `AgentName::try_new`.
- **Breaking:** **Notifications:** `SubscriptionView::source` is a `ChangeSource`, and the failure ledger's `stage` is a `FailureStage` (`NewFailure::stage`, `DeliveryFailure::stage`) in place of the `DELIVER_STAGE` and `DELIVER_RETRY_STAGE` constants. Migrate by building the typed values; a stored `source` or `stage` that does not parse is a decode error.

- **Breaking:** `NewNotifications::actor_display` is removed. Migrate struct literals by dropping the field.
## [0.76.0] - 2026-10-29

### Added

- Initial release: notification streams, rules, subscriptions, the per-user feed, at-least-once delivery through the outbox consumer, digests and coalescing, attention levels, retention, and the extension seams `NotificationRouter`, `NotificationAudienceResolver`, `PublishPolicy`, `ResourceVisibility` and `NotificationChannel`.
