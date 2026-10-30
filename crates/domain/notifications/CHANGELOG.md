# Changelog

## [0.77.0] - 2026-10-30

### Breaking

- **Breaking:** **Notifications:** `actor_display` and `excerpt` are removed from notifications; a notification carries `actor_user_id`, `title` and `body` (migration 004). Migrate feed clients by resolving the actor's name from the users API and rendering `title`/`body`.
- **Breaking:** **Notifications:** `PipelineParts::actors` and `NotificationActorDirectory` are removed, and `NewRecipient::excerpt` is replaced by `title` and `body`. Migrate struct literals by dropping the field and setting both.
- **Breaking:** **Notifications:** `via_agent` is an `AgentName` on `NewNotifications` and `Notification`. Migrate by constructing it with `AgentName::try_new`.
- **Breaking:** **Notifications:** `SubscriptionView::source` is a `ChangeSource`, and the failure ledger's `stage` is a `FailureStage` (`NewFailure::stage`, `DeliveryFailure::stage`) in place of the `DELIVER_STAGE` and `DELIVER_RETRY_STAGE` constants. Migrate by building the typed values; a stored `source` or `stage` that does not parse is a decode error.

## [0.76.0] - 2026-10-29

### Added

- Initial release: notification streams, rules, subscriptions, the per-user feed, at-least-once delivery through the outbox consumer, digests and coalescing, attention levels, retention, and the extension seams `NotificationRouter`, `NotificationAudienceResolver`, `PublishPolicy`, `ResourceVisibility` and `NotificationChannel`.
