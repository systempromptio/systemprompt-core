# systemprompt-notifications

Notification streams, routing rules, per-user feeds and the outbox consumer
for [systemprompt.io](https://systemprompt.io).

Producers append a `NotificationEvent` to the durable outbox through the
injected `NotificationSink`. The consumer routes each event to recipients,
applies rules, stream bands and resource visibility, persists one row per
recipient and delivers it on the registered channels.

Licensed under the Business Source License 1.1.
