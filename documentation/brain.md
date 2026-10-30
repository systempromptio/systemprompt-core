# Brain: Organisational Memory

How systemprompt-core remembers what an organisation has decided and puts that knowledge in front of its models: where facts come from, who confirms them, who may see them, and how each use of a fact is traced.

The brain lives in the `brain` domain (`crates/domain/brain`). Producers record events in the durable outbox in the same transaction as the change that caused them. A consumer reads each event and turns the event into candidate facts. A model, called through the in-process gateway client, extracts the candidates, and declared rules validate, deduplicate, route and confirm them. Every change to a fact is a revision. On each new user turn the gateway recalls the confirmed facts the caller may see and appends them to the request's system prompt as a `<team_memory>` block.

## Enabling

The brain is opt-in twice: at build time and in the profile.

Enable the `brain` feature of the `systemprompt` crate. `full` includes it and `core` does not. The feature implies `notifications`.

```toml
systemprompt = { version = "0.76", features = ["full"] }
# or, on top of a narrower set:
systemprompt = { version = "0.76", features = ["api", "brain"] }
```

Then switch it on in the profile:

```yaml
brain:
  enabled: true
```

`enabled` defaults to `false`. A server built without the feature, or running a profile with `enabled: false`, behaves as follows:

- No outbox consumer runs and no extraction model is called.
- The document store records no brain event. A revision lands and a comment resolves as usual.
- The gateway injects nothing. Each request's audit row records `skipped=disabled`.
- Built with the feature but disabled, every path under `/api/v1/brain` and `/api/v1/admin/brain` answers `404` with `error_key` `brain_disabled`. Built without the feature, those routes are not mounted and answer a plain `404`.
- The brain MCP tools cannot be composed; building them refuses with "set brain.enabled: true in the profile".

An enabled brain has two hard requirements, checked at start:

- **The gateway must be enabled.** Extraction calls models through the in-process gateway client. A profile with the brain on and the gateway off fails the start with "brain extraction requires the gateway to be enabled".
- **A user provider must be present.** The brain routes resolve each caller's current roles. Without a user provider, mounting the routes fails rather than serving routes that cannot authorize their callers.

## Guarantees

- **Fail closed per fact.** Each fact is checked on its own before it can reach a request: its scope, its topic audience, its granularity and the readability of its source. A source the reader cannot read, a source check that errors, and a source check that runs past `consumer.source_timeout_ms` all withhold the fact. A group membership no resolver can confirm is not membership. At capture, a source nobody can read rejects the candidate.
- **Fail open per request.** Memory never fails a request. A recall that runs past `recall.deadline_ms`, returns an error or finds no visible fact is skipped, and the request proceeds without memory. The skip and its reason are recorded on the audit row.
- **No agent in the loop.** An agent may search, explain, preview, propose and dispute. It never confirms, retracts, pins, erases or resolves. The MCP tools refuse those requests with `agent_forbidden`, and a proposal with any status other than `proposed` is refused the same way.
- **Models suggest, rules decide.** A model only produces candidates. Core's schema validator checks every candidate against its declared kind before any extension validator runs. Deduplication, routing and status are deterministic. A near-identical statement about the same subject opens a dispute and never overwrites the stored fact.
- **Every use is traceable.** Each recall that runs writes a `brain_recalls` row with the injected fact ids and revisions, the withheld counts by reason, the tokens spent and the exact rendered block. The gateway's `ai_requests.memory_recall` column carries a one-line descriptor of the decision, and the response carries the injected ids in `x-systemprompt-memory-ids`. The why-view explains any one fact's score.
- **Every change is a revision.** Creation, revision, merge, staling, confirmation, dispute, retraction, pinning and field edits each write a `brain_fact_revisions` row attributed to the actor.
- **Idempotent capture.** An event is processed at most once per pipeline version. A replayed observation writes no second outcome, because outcomes are unique per recall, fact and signal.
- **Bounded spend and storage.** Extraction stops for the day when the daily token budget is spent, and every table the brain writes has a retention bound.

## Concepts

### Facts

A fact is one revisioned statement about a subject. It carries a `kind`, a `subject`, its typed `fields`, a `statement` rendered from the kind's template, a `scope`, a `source`, its `topics`, a `status` and a `revision` number. A fact may be pinned, which places it in recall's pinned tier.

### Kinds

A kind is a declared type of fact: the subject kind it is about, its fields and their types, the statement template, the identity fields that decide when two candidates are the same fact, the materiality fields a candidate must carry, an optional review interval and an optional extractor. An undeclared kind cannot be entered or extracted. See [Declaring the brain](#declaring-the-brain).

### Subjects

A subject is the business entity a fact is about, written `<kind>:<id>`, for example `client:acme` or `service:billing`. Both halves must be non-empty. A subject key that does not parse is refused with `invalid_subject`.

### Scopes

| Scope | Key | Who sees the fact |
|---|---|---|
| `user` | A user id | Its owner only |
| `project` | A project name | Principals whose request carries that project attribution |
| `group` | A group name | Members of the `group` dimension, as a registered audience resolver confirms |
| `org` | None | Everyone in the organisation |

A topic audience can widen a fact's scope (see [Topics](#topics)). It never bypasses the source check.

### Sources

| Source | Captured from | Readability gate |
|---|---|---|
| `document` | A landed document revision, optionally citing a span | Read access to the live document |
| `comment` | A resolved comment | Read access to the comment's document |
| `entered_by` | A person or agent entering a fact | None |
| `session` | A conversation session | Hidden from everyone but its owner until confirmed |
| `run` | An agent run | Hidden until confirmed |
| `external` | An external system and reference | A `SourceAccess` that handles `external:<system>` |
| `aggregate` | A window of events on one pathway | None |

A document-sourced fact read by an agent is visible only when the document's agent cap for that agent, acting for the reader, is above `none`. Otherwise it is withheld as `agent_capped`.

The profile's `capture.sources` names four pathways: `document`, `comment`, `session` and `entered`. Core produces events on three of them: `document.revision.landed`, `document.comment.resolved` and `brain.fact.entered`.

### Topics

A topic is a controlled label with an `audience` and a `granularity`. A fact tagged with a topic is visible to every member of that topic's audience, in addition to its scope. A topic with `granularity: aggregate` (the default) shows a fact about a person (`user:<id>` subject) only to that person, unless the fact's source is an aggregate.

### Interests

An interest is a standing question with an owner. It matches new facts by kinds, topics, subjects, keywords and pathways; an empty predicate matches everything, and an expired interest matches nothing. An interest whose `delivery` includes `alert` sends its owner a `brain.interest.alert` notification for each matching fact.

### Workflows

A workflow profile is selected when its `id` equals the request's `project` attribution. It sets per-kind injection `quotas`, an optional `budget_tokens` cap, the `kinds` it cares about, and an optional `confirm_role` whose holders may govern facts of those kinds.

### Statuses

| Status | Meaning | Recalled |
|---|---|---|
| `candidate` | Entered without a request for review | No |
| `proposed` | Awaiting a person's confirmation | Only to its owner |
| `confirmed` | Accepted | Yes |
| `disputed` | Under an open dispute | No |
| `stale` | Its source changed since it was confirmed | Only with `recall.stale: marker`, suffixed `[stale: its source changed since it was confirmed]` |
| `superseded` | Replaced, or past its `retention_until` | No |
| `retired` | Retracted, or retired by an upheld dispute | No |

A confirmed fact past its review date is also withheld, as `review_due`.

Status transitions:

- An extracted fact starts `proposed`. An entered fact starts in the status its author chose, `candidate` (the default) or `proposed`. When the matching `capture.auto_confirm` policy is `always`, a new fact starts `confirmed`. `project_policy` names a per-project policy that no project declares, so it confirms nothing.
- A landed revision stales the facts built on earlier revisions of its document. A resolved comment stales the facts built on it.
- A dispute marks the fact `disputed`. Resolving the last open dispute as `dismissed` restores it to `confirmed`; an `upheld` resolution retires it.
- Retract retires the fact and keeps it. Erase deletes the fact and records a tombstone, so the same identity is never extracted or entered again.

### Pipeline version

The pipeline version is the first 16 hex characters of a SHA-256 over the declared configuration's hash, the brain code version and every kind's prompt version. Any change to any of them is a new version. Each event's result is recorded per pipeline version, and each extraction is recorded once per event and version.

## Declaring the brain

The brain's declarations are YAML under `services/brain/`. `systemprompt config apply` reconciles them into the database through the `brain` config plane. Without a `services/brain/` directory the database stays authoritative.

| Path | Holds |
|---|---|
| `topics.yaml` | The topic list |
| `filters.yaml` | Declared event filters |
| `importance.yaml` | The importance formula's weights |
| `retrieval.yaml` | Retrieval tier weights, fusion constant, fact cap and quotas |
| `kinds/*.yaml` | One fact kind per file |
| `interests/*.yaml` | One interest per file |
| `profiles/*.yaml` | One workflow profile per file |

Unknown fields are refused. All files merge into one set and are validated before anything is written:

- topic, kind, interest and workflow ids are unique;
- every topic and kind a declaration names is declared;
- a kind's statement template parses and names only declared fields (or `subject`);
- a kind declares a non-empty `identity`, and its identity and materiality fields are declared;
- a model extractor carries a non-empty `prompt_version`;
- every weight is between `0.0` and `1.0`, and `recency.half_life_days` is positive.

A kind:

```yaml
# services/brain/kinds/client_contact.yaml
kind: client_contact
subject: client                    # facts are about client:<id>
description: Who the client's primary contact is
fields:
  contact:   {type: text, required: true, max_len: 120}
  role:      {type: enum, values: [owner, sponsor, technical]}
  since:     {type: date}
  account:   {type: entity, ref: client}
topics: [accounts]
statement: "{subject}'s primary contact is {contact}{role? as {role}}"
identity: [subject]                # same subject = same fact, revised
materiality:
  require: [contact]
review_after_days: 60              # overrides lifecycle.review_after_days
extractor:
  type: model
  prompt: Extract the client's primary contact from the text.
  prompt_version: "1"
```

Field types are `text`, `entity`, `date`, `number`, `bool` and `enum`. A template substitutes `{name}` with a field and `{subject}` with the subject id; a group `{name? text}` is included only when `name` has a value. An extractor is `{type: model, prompt, prompt_version}` or `{type: parser, name}`.

Topics:

```yaml
# services/brain/topics.yaml
topics:
  - id: accounts
    name: Accounts
    description: Client relationships
    audience: [role:sales, group:emea]
    granularity: aggregate         # aggregate (default) or individual
```

Audience subjects use the notification forms: `user:<id>`, `role:<name>`, `<dimension>:<value>` or `everyone`.

An interest:

```yaml
# services/brain/interests/renewals.yaml
id: renewals
name: Renewal risks
owner: 0b7c2a1e-0000-4000-8000-000000000001
question: Which clients are at risk of not renewing?
pathways: [document_revision, comment_resolved]   # or entered
match:
  kinds: [client_contact]
  topics: [accounts]
  subjects: ['client:acme']
  keywords: [renewal, churn]
delivery: [alert]                  # memory, digest or alert
expires: 2027-06-30T00:00:00Z
```

A workflow profile:

```yaml
# services/brain/profiles/sales.yaml
id: sales                          # selected when the request's project is "sales"
name: Sales
kinds: [client_contact]
confirm_role: sales_lead
quotas: {client_contact: 4}
budget_tokens: 600                 # lowers recall.budget_tokens, never raises it
```

Filters:

```yaml
# services/brain/filters.yaml
filters:
  - name: account-docs
    pathways: [document_revision]
    actors: {humans: true, attended_agents: true, unattended_agents: false}
    kinds: [client_contact]
    debounce_secs: 60
```

Importance and retrieval, shown with their defaults where one exists:

```yaml
# services/brain/importance.yaml
base:
  kind_weight: {client_contact: 0.9}         # a missing kind counts as 1.0
  source_weight: {document: 1.0, entered_by: 0.8}
  model_hint: {field: confidence, weight: 0.2}
  confirmed_by_owner: 0.1
in_context:
  workflow_weight: {sales: {client_contact: 1.0}}
  subject_match: 0.3
  recency: {half_life_days: 90}
  usage: {cited_rate_weight: 0.2, ignored_penalty: 0.1}
```

```yaml
# services/brain/retrieval.yaml
tiers: {pinned: 1.0, subject: 1.0, context: 0.8, full_text: 0.6, extension: 0.0}
rrf_k: 60
max_facts: 12
quotas: {client_contact: 6}
```

Source weight keys are `document`, `comment`, `session`, `entered_by`, `external`, `run` and `aggregate`.

## Profile settings

The `brain` block of the profile is optional and every field has a default. Booleans are YAML `true` and `false`; `on` and `off` are refused.

```yaml
brain:
  enabled: true
  recall:
    budget_tokens: 800
    deadline_ms: 50
    scopes: [user, project, group, org]
    clients: {claude-code: true, claude-desktop: true}
    stale: withhold               # or marker
  capture:
    sources: [document, comment, session, entered]
    auto_confirm: {document: project_policy, others: never}
  extraction:
    model: claude-sonnet-4-5      # optional; the gateway's default when unset
    max_output_tokens: 2048
    timeout_ms: 60000
    daily_token_budget: 1000000
  lifecycle:
    review_after_days: 90
    retired_retention_days: 365
    recalls_retention_days: 90
    results_retention_days: 30
    correction_window_days: 14
  consumer:
    batch: 32
    poll_secs: 5
    sweep_secs: 300
    source_timeout_ms: 5000
```

| Field | Default | Effect |
|---|---|---|
| `enabled` | `false` | Master switch; see [Enabling](#enabling) |
| `recall.budget_tokens` | `800` | Token budget of one `<team_memory>` block; a workflow's `budget_tokens` can lower it |
| `recall.deadline_ms` | `50` | A recall past this is skipped as `deadline` |
| `recall.scopes` | all four | Scope levels recall searches |
| `recall.clients` | `claude-code: true`, `claude-desktop: true` | Clients recall runs for, keyed by `x-systemprompt-client` name; an unlisted client is off |
| `recall.stale` | `withhold` | `withhold` drops stale facts; `marker` injects them with the stale marker |
| `capture.sources` | all four | Pathways captured; an event on any other is ignored before the filters run |
| `capture.auto_confirm.document` | `project_policy` | Confirmation of document- and comment-sourced facts: `never`, `project_policy` or `always` |
| `capture.auto_confirm.others` | `never` | Confirmation of every other source |
| `extraction.provider` | None | Accepted by the schema |
| `extraction.model` | None | Model the extractor requests; unset uses the gateway's routing |
| `extraction.max_output_tokens` | `2048` | Output cap of each extraction call |
| `extraction.timeout_ms` | `60000` | Timeout of each extraction call |
| `extraction.daily_token_budget` | `1000000` | Extraction tokens per UTC day; once spent, events retry until the next day |
| `lifecycle.review_after_days` | `90` | Days until a new fact is due for review, unless its kind overrides it |
| `lifecycle.retired_retention_days` | `365` | Retired facts untouched this long are deleted |
| `lifecycle.recalls_retention_days` | `90` | `brain_recalls` rows older than this are deleted |
| `lifecycle.results_retention_days` | `30` | Event results, failures, extractions and outcome signals older than this are deleted |
| `lifecycle.correction_window_days` | `14` | A dispute, retraction or edit counts as a correction of recalls this recent |
| `consumer.batch` | `32` | Events claimed per drain round |
| `consumer.poll_secs` | `5` | Longest wait between rounds when no wake-up arrives |
| `consumer.sweep_secs` | `300` | Interval of the store sweep |
| `consumer.source_timeout_ms` | `5000` | Bound on each source read at extraction and each source check at recall |

The `brain_per_second` key of `rate_limits` (default `50`) bounds the brain routes.

## HTTP API

All `/api/v1/brain` routes require a signed-in, active user; an inactive account answers `401`. `/api/v1/admin/brain` routes require an admin. Each handler resolves the caller's current roles and the brain service authorizes the operation:

- Every read (listing, a single fact, its timeline, its why-view, a subject's facts) and every dispute applies the same per-fact visibility check recall uses: scope or topic audience, granularity, and whether the caller may read the fact's source as the actor the request acts as. A browser session acts as the person; any other credential acts as an agent. A fact the caller may not see is left out of listings and answers `404` `fact_not_found` when read directly, so its existence does not leak. A `project` fact is visible only when the request names that project (the `project` query parameter).
- A `user`-scoped fact may be governed and entered by its owner only.
- A `project`, `group` or `org` fact may be entered or governed (confirmed, retracted, erased, pinned, unpinned, edited, its disputes resolved) by an admin, or by a holder of the `confirm_role` of a workflow profile that lists the fact's kind. Without such a role the action is refused with `403`.
- Only an admin may list, decide and replay proposals.

| Method and path | Body or query | Success |
|---|---|---|
| `GET /api/v1/brain/facts` | Query: `kind`, `subject` (`kind:id`), `status` (comma-separated), `limit` (1 to 500, default 100), `project` | `200` with an array of facts |
| `POST /api/v1/brain/facts` | `{kind, subject, fields, scope, topics?, status?}`; `status` is `candidate` (default) or `proposed` | `202` with `{outbox_id}`; the consumer builds the fact |
| `GET /api/v1/brain/facts/{id}` | None | `200` with the fact |
| `GET /api/v1/brain/facts/{id}/timeline` | None | `200` with the fact's revisions |
| `GET /api/v1/brain/facts/{id}/why` | Query: `project`, `client` | `200` with `{fact, terms, withheld}` |
| `GET /api/v1/brain/subjects/{kind}/{id}` | None | `200` with the subject's facts |
| `POST /api/v1/brain/facts/{id}/confirm` | None | `200` with the revision |
| `POST /api/v1/brain/facts/{id}/dispute` | `{reason}` | `201` with `{dispute_id}` |
| `POST /api/v1/brain/facts/{id}/retract` | `{reason?}` | `200` with the revision |
| `POST /api/v1/brain/facts/{id}/erase` | None | `204` |
| `POST /api/v1/brain/facts/{id}/pin` | None | `200` with the revision |
| `POST /api/v1/brain/facts/{id}/unpin` | None | `200` with the revision |
| `PATCH /api/v1/brain/facts/{id}/fields` | `{fields}` | `200` with the revision; refused when the edit would change the fact's identity |
| `POST /api/v1/brain/disputes/{id}/resolve` | `{fact_id, outcome: upheld or dismissed, note?}` | `204` |
| `GET /api/v1/brain/proposals` | Query: `limit` | `200` with the open proposals |
| `POST /api/v1/brain/proposals/{id}/approve` | None | `204` |
| `POST /api/v1/brain/proposals/{id}/reject` | None | `204` |
| `POST /api/v1/brain/proposals/{id}/replay` | `{since, until}` (RFC 3339) | `200` with the dry-run report, also stored on the proposal |
| `POST /api/v1/admin/brain/simulate` | `{user_id, client, project?, prompt?, reuse?}` | `200` with `{descriptor, skipped?, block?}` |

`scope` is written `{level, key}`, for example `{"level": "project", "key": "sales"}` or `{"level": "org"}`. Unknown query parameters and body fields are refused. A field edit re-renders the statement from the kind's template. Deciding a proposal only records the decision; nothing a proposal describes applies itself.

### Simulation

`POST /api/v1/admin/brain/simulate` runs a new-turn recall for a chosen user, client and project, using `prompt` as the subject hints, and records nothing. The `block`, when present, carries the scope, `as_of`, the facts with their revisions, the withheld counts by reason, the tokens and the exact `rendered` text.

### Errors

| Status | `error_key` | When |
|---|---|---|
| `401` | None | No signed-in user, or the account is not active |
| `403` | `brain_forbidden` | The authority rules refuse the action, or the proposal is not open |
| `404` | `fact_not_found` | The fact does not exist |
| `404` | `proposal_not_found` | The proposal does not exist |
| `404` | `brain_disabled` | The brain is not enabled |
| `409` | `fact_tombstoned` | The identity was erased and may not be recorded again |
| `409` | `conflict` | A storage conflict |
| `422` | `unknown_kind` | The kind is not declared |
| `422` | `invalid_subject` | The subject is not `kind:id` |
| `422` | `invalid_statement` | The fields do not render the kind's statement |
| `422` | `invalid_status` | A `status` filter names no known status |
| `422` | `invalid_id` | A malformed id |
| `422` | `invalid_params` | Invalid arguments or data |
| `503` | `brain_unavailable` | Brain storage is unavailable; the body is a fixed message |
| `503` | `recall_unavailable` | The simulator's recall failed |
| `500` | None | Any other failure; the body is a fixed message and the cause is logged |

## MCP tools

An MCP server composes the brain tools as one set. Every call acts as the authenticated user, with the agent as its actor. A call without an authenticated user is refused with `forbidden`. Inputs refuse unknown keys. `brain_search` and `brain_why` apply the same per-fact visibility as the HTTP reads, for the agent: a fact whose source the agent may not read is left out or answers `not_found`. `brain_propose` needs the authority that governs the proposed scope.

| Tool | Input | Returns |
|---|---|---|
| `brain_search` | `query`, `kind?`, `subject?`, `scope?`, `limit?` (1 to 50, default 20) | `{facts}`: id, revision, kind, subject, statement, status, source |
| `brain_propose` | `kind`, `subject` (`kind:id`), `fields`, `topics?`, `scope?` (default: the caller's own), `status?` (only `proposed`) | `{outbox_id}` |
| `brain_dispute` | `fact_id`, `reason` | `{dispute_id}` |
| `brain_why` | `fact_id`, `project?` | The importance terms (`base`, `subject`, `workflow`, `recency`, `total`) and `withheld` |
| `brain_recall_preview` | `prompt`, `project?` | `injected`, `block` or `skipped`, `withheld` by reason, `tokens`; records nothing |
| `brain_guide` | `topics?`: `facts`, `scopes`, `recall`, `proposing`, `refusals` | The guide text |

A call to `brain_confirm`, `brain_approve`, `brain_retract`, `brain_pin`, `brain_erase` or `brain_resolve` is refused with `agent_forbidden`.

A refusal is a tool result with `isError: true` and `structuredContent` `{"code": ...}`:

| Code | Meaning |
|---|---|
| `brain_disabled` | The brain is off for this deployment |
| `unknown_kind` | The kind is not declared |
| `forbidden` | The fact or scope is not the caller's to see or enter into |
| `agent_forbidden` | Agents propose and dispute; a person confirms |
| `not_found` | No fact has that id |
| `tombstoned` | The fact was erased and may not be recorded again |
| `invalid_input` | An argument is malformed or an unknown key was sent |

A transient failure has no code. It is logged and answered with a fixed message as a protocol error.

## Gateway recall

### Headers

| Header | Direction | Meaning |
|---|---|---|
| `x-systemprompt-memory` | Request | `off`, in any case, disables recall for this request. Any other value or none leaves it on |
| `x-systemprompt-memory-ids` | Response | The injected facts as `<fact id>@<revision>`, comma-separated; absent when nothing was injected |

### When recall runs

The recall stage runs on every gateway request, before the upstream body is built. It is skipped when:

| Skip reason | When | `brain_recalls` row |
|---|---|---|
| `disabled` | No brain is composed | No |
| `header_off` | `x-systemprompt-memory: off` | No |
| `client_not_enabled` | The client is internal, or `recall.clients` does not enable it | No |
| `not_new_turn` | A continuing turn with no reusable stored block | Yes |
| `no_facts` | No visible fact survived | Yes |
| `deadline` | Recall ran past `recall.deadline_ms` | Yes |
| `unavailable` | Recall returned an error | Yes |

The brain's own extraction calls run with memory off.

### Same turn and new turn

A request that opens a new user turn runs a full recall. A request that continues a turn, such as a tool-result round trip, reuses the block the turn already sent. The stored block is accepted only when it names the same facts in the same order and renders to identical bytes, so the prompt cache stays warm, the ids header keeps its revisions and nothing changes mid-task.

### A new-turn recall

1. **Context.** The principal's user, client, actor and `project` attribution. A workflow profile whose `id` equals the project is selected.
2. **Retrieval.** Four tiers run concurrently, each scope-filtered in SQL for the principal: pinned facts, facts whose subject matches the last user message, the request project's facts by stored importance, and full-text matches over the last user message. The tiers are fused by weighted reciprocal rank (`retrieval.yaml`), then extension retrievers may boost.
3. **Lifecycle.** Stale (unless `marker`), review-due and out-of-validity facts are withheld, as are `candidate`, `disputed`, `superseded` and `retired` facts. A `proposed` fact reaches only its owner.
4. **Visibility.** Each remaining fact passes `(scope allows or topic audience contains) and granularity allows and source allows`, as described in [Concepts](#concepts).
5. **Fill.** An extension injection policy may choose the facts. Otherwise facts are taken in descending in-context importance while their kind is under its quota (`quota`), the count is under `max_facts`, and the estimated tokens (a quarter of the characters, rounded up) fit the budget (`budget`).

The block is appended after the client's system prompt:

```text
<team_memory scope="sales" as_of="2026-10-30T10:00:00Z">
- [<fact id> · doc:<document id>@<revision id>] Acme's primary contact is Jane Doe as sponsor
</team_memory>
```

The scope attribute is the project, or `org` when the request carries none. Withheld reasons are `scope`, `source`, `agent_capped`, `stale`, `review_due`, `quota`, `budget` and `unavailable`.

### Audit

`ai_requests.memory_recall` records one line per request: `injected=<n> withheld=<n> tokens=<n> as_of=<time>` or `skipped=<reason>`. It is `NULL` for requests that are not gateway requests. Each recorded recall writes a `brain_recalls` row with the request, conversation, session and context ids, the client, project and selected workflow, the injected `{id, revision}` list, the withheld counts, the tokens and the rendered block.

### Outcome signals

| Signal | Recorded when |
|---|---|
| `cited_in_response` | The assistant's response, buffered or assembled from the stream, names an injected fact id as a whole token |
| `user_corrected_fact` | A person disputes, retracts or edits a fact, against every recall that injected it within `lifecycle.correction_window_days` |

Extensions add their own signals through the `OutcomeSignal` seam. Each outcome is unique per recall, fact and signal.

## Consumer, jobs and health

### The consumer

Each round claims up to `consumer.batch` events and processes each on its own transaction. The event runs inside a savepoint and its outbox row is acknowledged in the same transaction, so one bad event never holds the others and a crash rolls back only the event in hand.

- An event that cannot succeed (it does not decode, its version is one the build does not read, or a stage refuses it for good) is recorded as a failure and acknowledged at once.
- An event that fails for a reason that may pass (storage, a seam call, a timeout, a spent token budget) is rolled back without acknowledgement, recorded, and retried after `poll_secs`. Its fifth failure is recorded as abandoned and acknowledged.

When nothing is pending the consumer listens on the durable facts channel, with `poll_secs` as a backstop. A lost listener reconnects with exponential backoff from one to thirty seconds. Every `sweep_secs` it sweeps the store under a transaction-scoped advisory lock, so one replica sweeps at a time, and publishes the store gauges.

For each event the pipeline normalises it, claims it once per pipeline version, stales the facts its source change replaces, runs the filters, extracts candidates, and for each candidate applies the rules, validates, deduplicates, routes, sets its status, persists and notifies. A new fact's notification commits or rolls back with the fact: `brain.fact.proposed`, `brain.fact.disputed` and `brain.interest.alert`, each with the fact's source as its subject, so recipients are gated by the source's visibility.

### The brain service account

Extraction calls models as the `brain` service account (`brain@service.invalid`), ensured on first start and created by the system admin. Its calls go through the in-process gateway client with an internal origin, a gateway session minted in-process and a `purpose` attribution, so they are metered, guarded, audited and failed over like any other gateway request. Its limits are the gateway policy's quotas on that account. Before each extraction call the consumer checks the day's spent tokens against `extraction.daily_token_budget`.

### Jobs

| Job | Schedule | Work |
|---|---|---|
| `brain_review` | Every 30 minutes | Marks facts past their review date as review-due, 500 at a time, one revision each, and sends `brain.fact.review_due` to whoever confirmed the fact, else its owner, else the `brain-review` stream |
| `brain_retention` | Every 10 minutes | See [Retention](#retention) |
| `brain_analysis` | Daily at 03:15 | Runs the core analyses and then every extension analysis over the last day, storing each draft no open proposal already carries |

The core analyses draft proposals; none applies itself:

- **Bloat**: a kind with many live facts that recall almost never injects. The draft retires the kind when nothing was injected, else raises its materiality bar.
- **Relevance**: injections against citations per workflow and kind. A kind cited rarely under a workflow gets a lower `importance.yaml` weight there, one cited often a higher one.
- **Coverage**: per project, recalls that ran and injected nothing. A project where most recalls come back empty gets a proposal to add a kind or a capture pathway.

A proposal can carry a dry-run replay: the brain consumer's events of a window are processed again under a chosen pipeline version on a transaction that is always rolled back, and each fact the replay would write is diffed against the stored one. Model calls in a replay are capped at 500 events and a quarter of the daily token budget.

### Metrics

| Series | Type | Labels |
|---|---|---|
| `brain_events_total` | Counter | `outcome`: `processed`, `ignored`, `no_filter`, `rejected`, `failed`, `abandoned`, `retry`, `decode_failed` |
| `brain_candidates_total` | Counter | `effect`: `created`, `revised`, `merged`, `unchanged`, `disputed`, `rejected`, `tombstoned` |
| `brain_extractions_total` | Counter | `extractor` |
| `brain_extracted_candidates_total` | Counter | `extractor` |
| `brain_budget_exhausted_total` | Counter | None |
| `brain_batch_seconds` | Histogram | None |
| `brain_facts` | Gauge | `status` |
| `brain_disputes_open` | Gauge | None |
| `brain_extraction_tokens_today` | Gauge | None |

### Report views

- `v_brain_kind_health`: per kind, facts by status, facts past review, injections, citations, corrections, the cited and corrected rates (`NULL` for a kind never injected) and open disputes.
- `v_brain_workflow_recall`: per project and client, with the selected `workflow_id`, recalls run and injecting, facts injected and withheld by reason, mean tokens per injecting recall, and recalls skipped by deadline, unavailability or an empty result.

## Not yet active in 0.77

These settings and declarations are accepted and validated, and have no effect in this release:

- **The session pathway.** `capture.sources` accepts `session`, but no producer emits session events, so nothing is captured from it.
- **`auto_confirm.document: project_policy`.** No project policy source exists yet, so it behaves as `never`: document facts wait for a person to confirm them.
- **`extraction.provider`.** Only `extraction.model` selects the extraction model; the provider is resolved by the gateway from the model.
- **`filters.yaml`.** The document is stored with the other declarations but the pipeline does not read it. Which events are gathered is decided only by registered `EventFilter` seams.
- **Consumer backlog in health.** The consumer's pending-event count is not part of the health detail; watch `brain_events_total{outcome="retry"}` and the outbox instead.
- **Interest fields.** `audience`, `granularity`, and the `memory` and `digest` delivery values are stored and have no effect; only `alert` delivery notifies.

## Retention

The `brain_retention` job runs every ten minutes. Each purge runs in batches of 1,000 under its own ten-second budget; what is left waits for the next run.

1. Retired facts not under legal hold, untouched for `retired_retention_days`, are deleted.
2. `brain_recalls` rows older than `recalls_retention_days` are deleted.
3. Event results, failures, extractions and outcome signals older than `results_retention_days` are deleted.
4. Facts past their `retention_until` and not under legal hold are taken out of recall as `superseded`, one revision each.

Erasing a fact deletes it and keeps a tombstone of its identity. Recalls are deleted with the user they were made for.

## Extension seams

Extensions add behaviour through compile-time registrations, each with a priority. Lists run in ascending priority.

| Seam | Registration | When it runs | Order and core default |
|---|---|---|---|
| `EventFilter` | `register_brain_filter!` | Per event, before any model call | The first `Gather` or `Ignore` wins; `Abstain` defers. Core registers none |
| `FactExtractor` | `register_brain_extractor!` | Per gathered kind it `handles` | Core's model extractor (`core.model`) serves every kind with a `model` extractor |
| `FactValidator` | `register_brain_validator!` | Per candidate, after core's schema validator (`core.schema`) | The first `Reject` or `Dispute` wins |
| `ImportanceModel` | `register_brain_importance!` | Stored base on write; in-context score per request | The first model that scores wins; core's formula (`core.formula`) is the fallback |
| `SourceContent` | `register_brain_source_content!` | Reading a source's text for extraction | The first that `handles` the source; core appends a document and comment reader |
| `FactRetriever` | `register_brain_retriever!` | Per new-turn recall, after the core tiers | Every retriever boosts the fused ranking; a failure skips its ranking. Core registers none |
| `InjectionPolicy` | `register_brain_injection_policy!` | Choosing which visible facts fill the budget | The first non-`None` plan wins; core's quota-and-budget fill is the fallback |
| `OutcomeSignal` | `register_brain_outcome_signal!` | After a response to a request with injected facts | Every signal runs after core's citation scan |
| `BrainAnalysis` | `register_brain_analysis!` | The daily `brain_analysis` job | Every analysis runs after the three core analyses |

Core registers no event filter, so with no extension filter registered no document or comment event is gathered and nothing is extracted. A person- or agent-entered fact bypasses the filters. Recall's source check uses core's document `SourceAccess`, which reads a document source as readable when the reader holds any level on the live document and hides every other subject kind.
