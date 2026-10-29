# Notifications and Subscriptions

How systemprompt-core tells people and agents that something happened: who hears about an event, how loudly, on which surface, and what they are asked to do about it.

Notifications live in the `notifications` domain (`crates/domain/notifications`). Producers record events in the same transaction as the change that caused them. A consumer turns each event into at most one row per recipient and announces each row on the registered channels at least once. Every surface (the web dashboard, the HTTP API, the desktop bridge and the documents MCP server) reads the same per-user feed.

## Enabling

Notifications are opt-in at build time. Enable the `notifications` feature of the `systemprompt` crate; `full` includes it and `core` does not.

```toml
systemprompt = { version = "0.76", features = ["full"] }
# or, on top of a narrower set:
systemprompt = { version = "0.76", features = ["api", "notifications"] }
```

A server built without the feature behaves as follows:

- The `/api/v1/notifications`, `/api/v1/admin/notifications` and `/v1/bridge/notifications` routes are not mounted and answer 404.
- No outbox consumer runs, and the documents MCP server does not list `documents_inbox`.
- A comment, mention or share commits and records no notification.
- A comment that sets `mention_options` is refused with `unsupported_op`, because the requested attention cannot be honoured.
- The desktop bridge shows that notifications are not enabled on the server and hides its inbox count.

## Guarantees

- **Transactional capture.** An event is appended to the durable outbox on the producer's own connection. A comment, share or publish never commits without its event, and an event never exists for a change that rolled back.
- **Nothing announced that can roll back.** Rows are written inside the consumer's transaction for that event. Channels run only after that transaction commits.
- **At-least-once announcement.** Every row records its `delivery_state` in the same transaction that writes it. A row moves to `delivered` only after every eligible channel has either accepted it or had its failure recorded. A timed sweep announces any row still `pending`, so a crash between commit and announcement delays a notification but never loses it.
- **Idempotent processing.** A re-delivered outbox fact produces no second row for a recipient who already has one.
- **The audience is a gate.** A stream reaches only members of its declared audience. A subscription never widens a stream's audience.
- **Fail closed.** A failure only ever makes delivery quieter or narrower, never louder or wider. A missing audience resolver, a resolver error, a resolver timeout or an audience above the configured limit withholds that audience whole. A membership no resolver can confirm withholds the recipient. A document, folder or comment the reader cannot see withholds the notification. An excerpt is shown only when visibility allows it. A rule whose role or dimension cannot be resolved may lower attention but never raise it. An unknown time zone in quiet hours counts as inside the window.
- **Every withholding is recorded.** Each withheld recipient and failed step is a row in `notification_delivery_failures` with its stage and a stable reason code. See [Failure records](#failure-records).
- **Bounded storage.** Feed reads write nothing, receipts are unique per notification, surface and state, unread counts stop at a fixed cap, and every table the pipeline writes has a retention bound.
- **Private to the recipient.** Realtime notification frames go to the recipient's own event stream only, never to a document's shared topic, so other viewers of a document do not learn who was notified.

## Concepts

### Events

An event names a `kind`, an actor, whether the actor is an agent (and which agent acted on whose behalf), the resources it concerns (`subjects`, each a `{kind, id}` pair), an optional stream, direct recipients, an optional sender-chosen attention, an excerpt and a free-form payload.

Kinds are dotted lowercase names. Core produces three:

| Kind | Produced when | Direct recipients | Default attention |
|---|---|---|---|
| `document.comment.mentioned` | A comment is posted or edited with `@mentions` | Each mentioned user except the author | `human`, or the attention the author chose per mention |
| `document.shared` | A document or folder is shared | Each user the grant reaches, except the sharer | `agent` |
| `stream.published` | Someone publishes to a stream | None; the stream audience and subscribers are reached | The stream's default |

Extensions may produce their own kinds (for example `deal.stage_changed`). Without a router of their own, such an event reaches its direct recipients at `silent`, or at the attention the producer set.

A producer appends **one event per action**. A comment that mentions several users is one event listing every mentioned user as a direct recipient, and a share to a role is one event for all of its members. The consumer fans the event out to recipients; producers never append one event per recipient.

### Attention levels

Every notification carries one of three levels, ordered `silent < agent < human`:

- **`silent`** records the event in the feed. Nothing is asked of anyone.
- **`agent`** is backlog for the user's agent. An agent working through the documents MCP server reads and acts on these items.
- **`human`** asks the person directly. The desktop bridge raises a toast for `human` items.

An agent may **escalate** an `agent` item to `human` with a note. The row keeps the level it was escalated from. Any other level cannot be escalated.

Each row records why it holds its level in `attention_reason`: the policy step that changed the level last. The values are `default`, `sender`, `stream_floor:<stream>`, `stream_ceiling:<stream>`, `rule:<id>`, `subscription_ceiling`, `quiet_hours` and `escalated`. A step that leaves the level unchanged leaves the reason unchanged. See [Policy order](#policy-order).

### Streams

A stream is a named channel with an audience, a list of publishers and an attention band:

- `audience` and `publishers` are lists of audience subjects: `user:<id>`, `role:<name>`, `<dimension>:<value>` for a dimension an extension registers (for example `group:sales`), or `everyone`.
- `attention.default` is the level the audience receives. `attention.floor` and `attention.ceiling` clamp every recipient before the recipient's rules apply.
- `retention_days` purges the stream's rows older than that, read or unread.
- `coalesce_window_secs` folds repeated events into one row (see below).

Only a declared publisher may publish. A role or dimension publisher is checked with a membership question to the registered audience resolvers; the role is never expanded to a member list for this check, and a membership the resolver cannot confirm refuses the publish.

The audience is a gate as well as a list. The recipients of a stream item are the declared audience and the stream's subscribers, restricted to members of the declared audience. For a subscriber outside the expanded audience, the consumer asks the resolvers whether that user is a member of any audience subject:

| Answer | Outcome | Failure reason |
|---|---|---|
| Member | Delivered | None |
| Not a member | Withheld | `not_in_audience` |
| Unknown, or no resolver handles the subject | Withheld | `audience_unknown` |

Resolvers are asked for at most `max_audience + 1` members of a subject. A subject with more than `max_audience` members is withheld whole and recorded once as `audience_over_limit`; it is never truncated. Every resolver call runs under `consumer.resolver_timeout_ms`, and a timeout withholds the subject as `audience_timeout`.

### Subscriptions

A subscription attaches an audience subject to a stream or to one resource (`{kind, id}`, for example a document). A subscription may carry an `attention_ceiling`, `quiet_hours` and a `digest` window that apply to the users it reaches. When several subscriptions reach one user, the lowest ceiling and the shortest digest win.

Users manage their own subscriptions. A user may subscribe to a stream only when the resolvers confirm the user is a member of the stream's audience; otherwise the request is refused with `403 not_in_audience` and nothing is stored. Subscriptions to a resource are not audience-gated; resource visibility decides each delivery.

Subscriptions for roles and dimensions are declared in configuration. See [Ownership](#ownership).

### Rules

A rule matches events and adjusts each recipient's delivery. Every non-empty predicate must hold:

| Predicate | Matches |
|---|---|
| `kinds` | Glob patterns over the kind, for example `document.*` |
| `streams` | The event's stream |
| `resources` | Any of the event's subjects |
| `actors` | The actor as a user, role or dimension member |
| `actor_is_agent` | Whether an agent acted |
| `recipients` | The recipient as a user, role or dimension member |

A matching rule's action applies in this order: `set` replaces the level, `cap` lowers it to at most, `floor` raises it to at least, and `mute` withholds the notification and stops further rules. An action may also set `coalesce_window_secs`, a `digest` window and `quiet_hours`.

Declared rules apply first, by ascending `priority`. The recipient's own rules apply after them, to that recipient only. A rule that depends on a role or dimension whose membership could not be resolved applies only when it would not raise the level.

### Policy order

Each recipient's level is decided by these steps, in this order. `attention_reason` names the last step that changed the level.

1. **Core default.** The default router's level for the kind, or the stream's `attention.default` for a stream item.
2. **Sender attention.** The attention the producer chose for the event or for one recipient.
3. **Extension routers.** Registered routers add recipients; when two plans name one recipient, the louder level wins.
4. **Stream floor and ceiling.** The stream's band clamps the level.
5. **Recipient rules.** Declared rules, then the recipient's own rules (`set`, `cap`, `floor`, `mute`).
6. **Subscription ceiling.** The lowest `attention_ceiling` of the subscriptions that reach the recipient lowers the level to at most that.
7. **Quiet hours.** An open quiet-hours window from a matching rule or a reaching subscription lowers the level to at most `agent`.

Because the stream band applies before the rules, a recipient's own `cap` can lower an item below the stream's floor. Because the subscription ceiling and quiet hours apply last, no rule can raise an item above them.

### Quiet hours, coalescing and digests

- **Quiet hours** (`start`, `end`, `tz`, optional `weekdays`) cap a recipient at `agent` while the window is open. `tz` is `UTC` or a fixed offset such as `+01:00`; any other value counts as inside the window. A window whose end is before its start runs overnight. Weekdays are evaluated in the window's own time zone.
- **Coalescing** folds events into the recipient's existing unread row when they share a coalesce key: `<kind>:<stream>:<primary subject kind>:<primary subject id>:<actor>:<bucket>`, with `-` for an absent stream or subject. The bucket is the event time divided by the window. Events from two actors never share a key, so one row never mixes whose event it announces. A fold updates the excerpt, subjects, action prompt, `last_event_at` and `coalesce_count` together, so the row always describes its latest event. Without a window the bucket is the outbox id and every event keeps its own row. The window comes from a matching rule, else from the stream.
- **A digest** holds a row until the end of the digest window its event falls in. A held row is in the feed with `delivery_state` `held` but is not pushed to any channel until the sweep releases it after the window closes.

### Excerpts

An event's excerpt is denied by default. A recipient's row carries the excerpt only when one of these holds:

- every subject of the event is visible to the recipient with its excerpt allowed by that subject's `ResourceVisibility` provider; or
- the event has no subjects and the recipient passed the stream's audience gate.

A subject kind with no visibility provider is visible without its excerpt, except the document store's own kinds (`document`, `folder`, `comment`), which are withheld with `no_visibility_provider`. A subject hidden from the recipient withholds the whole notification with `hidden`.

### Delivery

Rows commit with `delivery_state` `pending`, or `held` when a digest applies. After the commit the consumer hands each `pending` row to every registered channel that delivers its attention and has not already recorded a `delivered` receipt for it, eight rows at a time. Each channel call runs under `consumer.channel_timeout_ms`. A success writes a `delivered` receipt for that surface. A failure or timeout writes a `deliver:<surface>` failure row. When every eligible channel has one or the other, the row moves to `delivered`.

Every `consumer.sweep_secs` one replica, holding a Postgres advisory lock, runs the sweep. It releases `held` rows whose window has closed and announces `pending` rows older than `sweep_secs`, at most `consumer.batch` of each per pass. A row whose announcement was interrupted is therefore retried. A channel may see the same row twice after a crash between its call and the receipt, so channels treat the notification id as idempotent.

### Receipts

Each surface records what happened to a notification in `notification_receipts`: `delivered` when a channel pushed it, `seen` when a client reports it displayed the row, `read`, `acted` and `escalated`. The surface is one of `agui`, `dashboard`, `api`, `bridge` or `mcp`. A receipt is unique per notification, surface and state, so repeating an action records nothing new.

Reading the feed writes nothing. A client that displays rows reports them through `POST .../seen`, which records `seen` receipts only for rows the caller owns.

### Ownership

Every stream, declared rule and subscription row records its source: `code:<file>` for a declaration from `services/notifications/`, or `console:<user>` for a user's own subscription or rule. An update never changes a row's source.

- A user's write that would change a `code:` row is refused with `409 declared_by_config`. This covers subscribing to a target a declared subscription already holds for that user, and setting an own rule whose id a declared rule holds.
- `systemprompt config apply` never adopts, overwrites or deletes a `console:` row. A declaration that collides with a user's own subscription fails the apply and names the entity.

## Declaring streams, rules and subscriptions

Streams, declared rules and declared subscriptions are YAML under `services/notifications/`. Every file in the directory merges into one declaration set. `systemprompt config apply` reconciles it into the database through the `notifications` config plane. Unknown fields are refused, and the set is validated before anything is written:

- stream and rule ids are unique;
- a stream declares at least one audience subject and one publisher;
- a stream's floor is not above its ceiling, and a rule's floor is not above its cap;
- every stream a rule or subscription names is declared.

A complete example:

```yaml
# services/notifications/streams.yaml
streams:
  - id: sales-news
    name: Sales news
    description: Announcements for the sales team
    audience: [role:sales, group:emea]
    publishers: [role:sales_lead]
    attention:
      default: agent
      floor: silent
      ceiling: human
    retention_days: 30
    coalesce_window_secs: 300
  - id: ops
    name: Operations
    audience: [everyone]
    publishers: [role:admin]
```

```yaml
# services/notifications/rules.yaml
rules:
  - id: 0d7f4e1c-1a2b-4c3d-8e9f-001122334455
    name: Agents never interrupt
    priority: 10
    match:
      kinds: ['document.*']
      actor_is_agent: true
    action:
      cap: agent
  - id: 6a1e2f3b-4c5d-4e6f-9a0b-66778899aabb
    name: Quiet weekends on sales news
    priority: 20
    match:
      streams: [sales-news]
      recipients: [role:sales]
    action:
      digest: {every_secs: 3600}
      quiet_hours:
        start: '22:00'
        end: '07:00'
        tz: '+01:00'
        weekdays: [Sat, Sun]
```

```yaml
# services/notifications/subscriptions.yaml
subscriptions:
  - subject: role:support
    target: {stream: ops}
    attention_ceiling: agent
  - subject: group:sales
    target:
      resource: {kind: document, id: 3f1c2b4a-0000-4000-8000-000000000001}
    digest: {every_secs: 900}
```

Stream ids are 1 to 64 characters of lowercase letters, digits, `_` and `-`. Rule ids are UUIDs.

## Profile settings

The `notifications` block of the profile is optional. Every field has a default.

```yaml
notifications:
  retention_days: 90           # read rows older than this are purged
  max_audience: 500            # largest audience subject one event may expand
  unread_max_age_days: 180     # unread rows older than this are deleted
  failure_retention_days: 30   # failure records older than this are deleted
  consumer:
    batch: 64                  # facts per drain round; rows per sweep pass
    poll_secs: 5               # backstop poll when no wake-up signal arrives
    sweep_secs: 30             # sweep interval, and the age a pending row must reach to be swept
    resolver_timeout_ms: 5000  # bound on each audience resolver call
    channel_timeout_ms: 5000   # bound on each channel call
```

| Field | Default | Effect |
|---|---|---|
| `retention_days` | `90` | Read rows older than this are deleted |
| `max_audience` | `500` | A subject with more members is withheld whole as `audience_over_limit` |
| `unread_max_age_days` | `180` | Unread rows older than this are deleted |
| `failure_retention_days` | `30` | `notification_delivery_failures` rows older than this are deleted |
| `consumer.batch` | `64` | Outbox facts per drain round, and rows per sweep pass |
| `consumer.poll_secs` | `5` | Longest wait between drain rounds when no wake-up arrives |
| `consumer.sweep_secs` | `30` | Sweep interval and the grace before a `pending` row is swept |
| `consumer.resolver_timeout_ms` | `5000` | A resolver call past this withholds its subject as `audience_timeout` |
| `consumer.channel_timeout_ms` | `5000` | A channel call past this records `channel_timeout` |

The `notifications_per_second` key of `rate_limits` bounds the notification routes.

## HTTP API

All `/api/v1/notifications` routes require a signed-in user and act only on that user's rows, subscriptions and rules. Errors use the common envelope with an `error_key`.

| Method and path | Body | Success |
|---|---|---|
| `GET /api/v1/notifications` | Query: `since` (cursor), `limit` (1 to 100, default 50), `attention`, `unread` | `200` with `{notifications, next_cursor, unread, unread_capped}` |
| `GET /api/v1/notifications/unread` | None | `200` with `{unread, unread_capped}` |
| `POST /api/v1/notifications/seen` | `{ids: [...]}` | `200` with `{seen}`, the number of new `seen` receipts |
| `POST /api/v1/notifications/read` | `{ids: [...]}` or `{all: true}` | `200` with `{ids, unread}` |
| `POST /api/v1/notifications/{id}/escalate` | `{note}` (optional) | `200` with the escalated notification |
| `GET /api/v1/notifications/streams` | None | `200` with `{streams}` |
| `POST /api/v1/notifications/streams/{id}/publish` | `{title, body?, subjects?, attention?}` | `202` with `{enqueued: true}` |
| `GET /api/v1/notifications/subscriptions` | None | `200` with `{subscriptions}` |
| `POST /api/v1/notifications/subscriptions` | A subscription; the subject is always the caller | `201` with the subscription |
| `DELETE /api/v1/notifications/subscriptions/{id}` | None | `204` |
| `GET /api/v1/notifications/rules` | None | `200` with `{rules}` |
| `PUT /api/v1/notifications/rules` | `{rules: [...]}`, replacing the caller's own rules | `204` |
| `POST /api/v1/admin/notifications/simulate` | `{event}` | `200` with `{plan, decisions}`; admin only |

The feed is newest first. `next_cursor` is present only when the page is full. Pass it back as `since` for the next page. A cursor is opaque. Reading the feed records nothing.

`unread` counts at most 1,000 rows. When more are unread, `unread` is `1000` and `unread_capped` is `true`; clients show the count as "1000+".

Each notification carries `delivery_state`: `pending`, `held` or `delivered`.

`POST .../seen` ignores ids that do not exist or belong to another user, and an id already recorded as seen. Calling it again with the same ids answers `{"seen": 0}`.

| Status | `error_key` | When |
|---|---|---|
| `400` | None | A malformed cursor or id |
| `400` | `invalid_declaration` | A rule set with duplicate ids or a floor above its cap |
| `401` | None | No signed-in user |
| `403` | `publish_refused` | The caller is not a declared publisher of the stream |
| `403` | `not_in_audience` | The caller is not a member of the stream's audience and cannot subscribe to it |
| `404` | `notification_not_found` | The notification does not exist or belongs to another user |
| `404` | `stream_not_found` | The stream is not declared |
| `409` | `not_escalatable` | The notification is not at `agent` attention |
| `409` | `declared_by_config` | The write would change a subscription or rule declared in configuration |
| `500` | None | A server-side failure; the body is a fixed message and the cause is logged |

### Simulation

`POST /api/v1/admin/notifications/simulate` runs routing, audience expansion, rules, bands and visibility for an event without writing anything. For every planned recipient it answers the attention the recipient would get (or `null` and a `withheld_reason`), the reason and the rules that matched. Processing the same event then produces exactly those rows.

## Realtime events

Each user's AG-UI event stream carries three custom events: `notification.created` (the row and the unread count), `notification.read` (the ids and the unread count) and `notification.escalated` (the id and the new attention). Clients re-read the feed on reconnect, so a missed frame is never lost.

## Desktop bridge

The bridge reads the same feed under `/v1/bridge/notifications`, authenticated with the bridge's own credentials, and records the `bridge` surface:

| Method and path | Purpose |
|---|---|
| `GET /v1/bridge/notifications` | The feed, with the same query parameters |
| `POST /v1/bridge/notifications/seen` | Record the rows the bridge displayed as seen |
| `POST /v1/bridge/notifications/read` | Mark read |
| `POST /v1/bridge/notifications/{id}/escalate` | Escalate |
| `GET /v1/bridge/notifications/streams` | Streams |
| `GET`, `POST /v1/bridge/notifications/subscriptions` | List and create subscriptions |
| `DELETE /v1/bridge/notifications/subscriptions/{id}` | Remove a subscription |

The bridge keeps a local inbox, catches up from its stored cursor whenever its event stream reopens, and raises a desktop toast for `human` items.

## Documents MCP server

The `documents_inbox` tool lets an agent work its user's feed, recorded against the `mcp` surface. Its `action` is `list` (with `since`, `limit`, `attention` and `unread`), `mark_read` (with `ids` or `all`) or `escalate` (with `id` and `note`). Every notification carries an action prompt: an instruction worded for its kind, and for a document event a link to the document or comment. Items at `agent` attention are the agent's backlog.

## Retention

The retention job `notification_retention` runs every five minutes on one replica. It runs three purges, each in batches of 1,000 rows under its own ten-second budget, so a large backlog in one never starves the others:

1. Read rows older than the profile's `retention_days`, and every row of a stream older than that stream's own `retention_days`, read or unread.
2. `notification_delivery_failures` rows older than `failure_retention_days`.
3. Unread rows older than `unread_max_age_days`. These are deleted rather than marked read, because a row nobody read in that time carries no further signal and a read row would only wait for the first purge.

Receipts are deleted with their notification. Deleting a user removes their notifications, receipts, subscriptions and recorded failures.

## Consumer and health

The consumer processes each outbox fact on its own transaction and acknowledges the fact in that transaction, so one large fact never holds the others. A fact that fails, or whose version the build does not read, is rolled back, recorded as a `decode` or `process` failure and acknowledged, so it never blocks the queue.

Between rounds the consumer listens on the Postgres channel `systemprompt_notification_facts`, which carries only notification facts. The `poll_secs` interval is a backstop. A lost listener reconnects with exponential backoff from one to thirty seconds while the consumer keeps polling.

`GET /health/detail` reports the consumer under `checks.notifications`:

```json
{"status": "healthy", "state": "listening", "pending": 0, "oldest_seconds": null}
```

`state` is `idle`, `draining`, `listening` or `reconnecting`. `pending` is the number of rows waiting for announcement and `oldest_seconds` the age of the oldest one. A backlog that cannot be read reports the entry as `degraded`.

### Metrics

| Series | Type | Labels |
|---|---|---|
| `notifications_facts_total` | Counter | `outcome`: `processed`, `failed`, `decode_failed`, `unsupported_version`, `ack_failed` |
| `notifications_recipients_total` | Counter | `decision`: `inserted`, `coalesced`, `held`, `muted`, or the failure reason that withheld the recipient |
| `notifications_deliveries_total` | Counter | `surface`, and `status`: `delivered`, `failed`, `timeout` |
| `notifications_batch_seconds` | Histogram | None; one sample per drain round |
| `notifications_backlog_pending` | Gauge | None |
| `notifications_backlog_oldest_seconds` | Gauge | None; `0` when nothing is pending |

### Failure records

| Stage | Reasons |
|---|---|
| `route` | `<router>: <error>` |
| `audience` | `not_in_audience`, `audience_unknown`, `audience_over_limit`, `audience_timeout`, `no_resolver`, `audience_resolver_failed`, `unknown_stream <id>` |
| `rules` | `audience_over_limit`, `audience_timeout`, `no_resolver`, `audience_resolver_failed` |
| `visibility` | `hidden`, `no_visibility_provider`, `visibility_failed` |
| `decode`, `process` | The decode or processing error |
| `deliver:<surface>` | `channel_timeout`, or the channel's error |

A recipient-specific failure names the recipient. A failure about a whole subject, such as `audience_over_limit`, is recorded once with no recipient.

## Extension seams

Extensions add behaviour through compile-time registrations, each with a priority:

| Seam | Registration | Purpose |
|---|---|---|
| `NotificationRouter` | `register_notification_router!` | Plan recipients for an event; plans merge and the louder decision wins |
| `NotificationAudienceResolver` | `register_notification_audience!` | Expand a role or dimension subject to users |
| `PublishPolicy` | `register_notification_publish_policy!` | Admit or refuse a publisher; every policy must admit |
| `ResourceVisibility` | `register_resource_visibility!` | Decide whether a reader may see a resource kind and its excerpt |
| `NotificationChannel` | `register_notification_channel!` | Deliver committed rows to another surface, such as email |

Core registers a default router, a resolver for users, roles and `everyone`, the declared-publishers policy, document-store visibility and the AG-UI channel. No email channel ships in core.
