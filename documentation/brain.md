# Brain: Organisational Memory

How systemprompt-core remembers what an organisation has decided and puts that knowledge in front of its models: where facts come from, who confirms them, who may see them, and how each use of a fact is traced.

The brain lives in the `brain` domain (`crates/domain/brain`). Producers record events in the durable outbox in the same transaction as the change that caused them. A consumer reads each event and turns the event into candidate facts. A model, called through the in-process gateway client, extracts the candidates, and declared rules validate, deduplicate, route and confirm them. Every change to a fact is a revision. On each new user turn the gateway recalls the confirmed facts the caller may see and appends them to the request's system prompt as a `<team_memory>` block.

This page is the reference: what to declare, what each knob does, what every surface returns. The recall path as an architecture, with its invariants and what is measured, is [concepts/brain-recall.md](concepts/brain-recall.md).

## Enabling

The brain is opt-in twice: at build time and in the profile.

Enable the `brain` feature of the `systemprompt` crate. `full` includes it and `core` does not. The feature implies `notifications`.

```toml
systemprompt = { version = "0.81", features = ["full"] }
# or, on top of a narrower set:
systemprompt = { version = "0.81", features = ["api", "brain"] }
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

A subject is the business entity a fact is about: a subject kind and a key within it, for example `client:acme` or `service:billing`. Both halves must be non-empty. On the wire its form depends on the surface. `POST /api/v1/brain/facts` takes an object, `{"kind": "client", "id": "acme"}`; a `client:acme` string there is refused with `422`. Query parameters, `brain_search` and `brain_propose` take the `kind:id` string, and one that does not parse is refused with `invalid_subject`.

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
| `retrieval.yaml` | Retrieval tier weights, fusion constant, the recall rules and their knobs, fact cap and quotas |
| `synonyms.yaml` | Groups of terms the search index treats as one |
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

Field types are `text`, `entity`, `date`, `number`, `bool` and `enum`. A field may also declare `alias: true`: its value is written as an alias of the fact's subject, so a later mention by that name resolves to the subject. A template substitutes `{name}` with a field and `{subject}` with the subject id; a group `{name? text}` is included only when `name` has a value. An extractor is `{type: model, prompt, prompt_version}` or `{type: parser, name}`.

Two further declarations make a kind askable and its entities subjects in their own right:

```yaml
# services/brain/kinds/team_assignment.yaml
kind: team_assignment
subject: project
fields:
  person: {type: text, required: true, max_len: 120}
  user:   {type: entity, ref: user, display: person}   # the person as a subject, named by `person`
  role:   {type: enum, required: true, values: [engagement lead, developer, project manager]}
statement: "{person} is the {role} on {subject}"
identity: [subject, person]
materiality: {require: [person, role]}
asks: [team, staffed, assigned, lead, manager, role]  # a message carrying one of these asks this kind
```

- **`asks`** lists the kind's *ask words*: lower-case single tokens (`BrainDeclError::AskNotToken` otherwise). A message asks a kind when one of its alphanumeric tokens is a word of the kind id or of this list; without the list, the id words alone. An asked kind is a recall signal and its facts rank first in the context and subject tiers (see [A new-turn recall](#a-new-turn-recall)).
- **`display`** on an `entity` field names a `text` field of the same kind (`BrainDeclError::DisplayNotTextField` when it sits on another type or names a non-text field). Every value the entity field takes (`<ref>:<id>`) becomes a `brain_subjects` row of kind `<ref>` with that text as its display name, aliased by its id, its id with `-` and `_` as spaces, and the display name, so a person a team fact names matches like a client and the `project <-> user` bridge the fact declares has a subject to anchor on.

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
owner: jane@example.com            # a user id or an email
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

`owner` is the user the interest's alerts go to, given as a `users.id` or as that user's email. An email keeps a tree portable across instances, whose user ids differ. `config apply` resolves it to the user's id when it writes the interest and refuses the apply with a validation error when no user has that id or email. The stored declaration keeps the value as written, so a shared tree hashes the same everywhere.

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

Importance and retrieval. The importance values below are an example, not the defaults: without `importance.yaml` every weight is `0.0` except `recency.half_life_days`. With `subject_match` and `confirmed_by_owner` at `0.0`, a subject hit raises a fact in the fused retrieval order but not in the fill order, which follows in-context importance.

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

Retrieval. Every knob below is optional and shown at its core default, so the file as written changes nothing; each line is the rule a knob declares, and [A new-turn recall](#a-new-turn-recall) is where the rule runs. The types and bounds are `RetrievalDoc` (`crates/shared/models/src/brain/declarations/importance.rs`); a weight outside `0.0..=1.0`, a zero limit or a stop word that is not one lower-case token is refused at apply.

```yaml
# services/brain/retrieval.yaml
tiers: {pinned: 1.0, subject: 1.0, linked: 0.6, context: 0.8, full_text: 0.6, extension: 0.0}
rrf_k: 60                               # reciprocal-rank-fusion constant
max_facts: 12                           # most facts one block carries
quotas: {}                              # per-kind cap on injected facts, e.g. {client_contact: 6}

# The subjects a message names
alias_min_sim: 0.6                      # least alias similarity that names a subject; 0.6 is the trigram floor, lower admits nothing more
alias_longest_wins: false               # drop an exact match whose every alias sits inside another matched subject's alias
max_subjects: 4                         # most subjects one message names
subject_limit: 0                        # subject and linked tiers' row limit; 0 = max_facts x 2 clamped to 8..=50; at most 100

# The linked tier
links_weight: 0.6                       # a fact linked to a named subject scores this share of the subject's score
links_depth: 1                          # hops walked from a named subject, 1 or 2; 2 crosses the declared subject bridges
links_decay: 0.5                        # the further share a depth-2 hit takes (0..=1)
links_require_readable_anchor: false    # true: a matched subject with no fact the reader can read bridges nothing

# The full-text planner
search_max_df: 0.2                      # a word in more than this share of the facts only ranks, never filters
search_quorum: 0.5                      # share of the informative words, rarest first, a fact must carry (at least one)
search_retry_quorum: 0.0                # a second pass at this quorum when the first returns nothing; 0 = none
search_quorum_over: groups              # count the quorum over synonym groups, or over the query words they absorbed
search_stop_words: []                   # lower-case tokens dropped before expansion and quorum
search_max_ndoc: 0                      # most rows the filter may match before the planner plans nothing; 0 = off
search_rare_alone_share: 0.0            # a word at or under this share is required alone, whatever the quorum; 0 = off

# Signal rules
context_requires_signal: false          # true: with no named subject, no full-text hit and no asked kind, context-only facts are withheld
full_text_opens_context: true           # false: a full-text hit alone does not count as the signal above
unknown_name_withholds_full_text: false # true: a capitalised word no fact carries empties the full-text tier

# Extensions
extension_max_candidates: 24            # most facts extension retrievers may introduce per recall
reranker_timeout_ms: 150                # bound on the first registered Reranker; past it the fused order stands

# The fill
fill_order: rank                        # rank = fused score, then importance; importance = the pre-0.78 order
linked_share: 0.0                       # share of max_facts held for linked hits when a subject is named
min_fused_score: 0.0                    # a fact fused below it is withheld as low_score
context_share: 1.0                      # share of max_facts context-only facts may take when a subject is named
usage_weight: 0.0                       # usage_weight x (cited_rate - ignored_rate) is added to each fused score
usage_min_recalls: 0                    # injections before brain_usage rates a fact; under it the fact keeps the prior
```

Source weight keys are `document`, `comment`, `session`, `entered_by`, `external`, `run` and `aggregate`.

Synonyms:

```yaml
# services/brain/synonyms.yaml
groups:
  - [invoice, bill, statement of account]
  - [client, customer]
```

Every group needs at least two non-blank members. The groups expand at index time, not at query time: when any member occurs in a fact's text (whole words, case-insensitive; a multi-word member as a phrase), every member of that group is appended to the text indexed at the lowest search weight. Applying a changed `synonyms.yaml` rewrites every fact's weighted vector in the same apply transaction. Without the file there are no groups.

### What a write keeps current

Every fact write (a capture insert, a revision, a merge, a field edit) refreshes the derived indexes below on the same connection, in the same transaction, from the stored fact, its kind declaration and the topics table, so a console edit and a captured candidate index identically. The reindex is set-based (`index::batch`): one read for the set, then one statement per index over the whole set, eleven for a set of any size.

| Index | Holds | Changes when |
|---|---|---|
| `brain_working_set` | One row per fact per scope key it is reachable through: its own scope (`org`, `user:<id>`, `project:<key>`, `group:<key>`) and each audience subject of its topics (`role:<r>`, `user:<id>`, `<dimension>:<value>`; `everyone` folds into `org`), with its `importance_base` | Insert; a revision of scope, topics or importance; a topic's audience (the apply rebuilds the table); delete (the foreign key cascades). Status, review, validity and pinning stay on `brain_facts` and are filtered at read |
| `brain_fact_links` | The subjects a fact mentions: its own subject via `subject`, and each `entity` field's value via that field's name (`<kind>:<id>` as written, or the field's declared `ref` kind with a bare value) | Insert, revision, field edit; a kind's fields (the apply rebuilds) |
| `brain_subject_links` | The bridges a fact declares: for every fact whose subject and one of whose entity fields name two different subjects, one row each way (`subject -> other`, `other -> subject`, `via_kind` the fact's kind), derived from the links just written. The depth-2 linked tier walks them | With the links; a kind's fields (the apply rebuilds). Migration `008_subject_links` queues every fact with an entity link for the `brain_fact_index` projection, and the sweep writes its bridges as it drains |
| `brain_subjects` (entity subjects) | A row of kind `<ref>` for every `entity` field value `<ref>:<id>` whose field declares `display`, named by that text field's value (`index::entities`) | Insert, revision, field edit |
| `brain_aliases` | For the fact's subject: the id, the id with `-` and `_` as spaces, the subject's display name, and every `alias: true` field value. For each entity subject above: its id, its id with `-` and `_` as spaces, and its display name | Insert, revision, field edit; an alias moves to the latest subject written with it |
| `brain_facts.search_weighted` | `A`: the subject id and identity field values; `B`: every other field value; `C`: the statement plus its synonym expansions | Insert, revision, field edit; a kind or the synonyms document (the apply rebuilds) |

The sweep (`consumer.sweep_secs`) compares the working set's row count with the count the facts and topics imply and rebuilds the table when they differ, counting `brain_working_set_rebuilds_total`.

Every fact also carries `observed_at`, when it was observed to hold: the `observed_at` an entered fact names, else the insert time. On upgrade the consumer sweep floors it to `created_at` for rows that predate the column, as it drains the `brain_fact_index` backlog the migration queues. An `as_of` read (`GET /api/v1/brain/facts?as_of=`, the simulator and `brain_recall_preview`) keeps only facts observed at or before that instant and valid then.

## Profile settings

The `brain` block of the profile is optional and every field has a default. Booleans are YAML `true` and `false`; `on` and `off` are refused.

```yaml
brain:
  enabled: true
  recall:
    budget_tokens: 800
    deadline_ms: 50
    source_timeout_ms: 40
    scopes: [user, project, group, org]
    clients: {claude-code: true, claude-desktop: true}
    stale: withhold               # or marker
  capture:
    sources: [document, comment, entered]
    auto_confirm: {document: project_policy, others: never}
    session: {enabled: false, max_chars: 4000}   # scaffolding; see Scaffolding
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
  consolidation:
    enabled: false                 # scaffolding; see Scaffolding
    daily_token_budget: null       # falls back to extraction.daily_token_budget
```

| Field | Default | Effect |
|---|---|---|
| `enabled` | `false` | Master switch; see [Enabling](#enabling) |
| `recall.budget_tokens` | `800` | Token budget of one `<team_memory>` block; a workflow's `budget_tokens` can lower it |
| `recall.deadline_ms` | `200` | A recall past this is skipped as `deadline`; the engine keeps it running for the turn's next request (see [Deadline](#deadline)). Measured in the 2026-11-02 proving run |
| `recall.source_timeout_ms` | `40` | Bound on each source check at recall; a check past it withholds the fact as `source`. Keep it under `recall.deadline_ms` |
| `recall.scopes` | all four | Scope levels recall searches |
| `recall.clients` | `claude-code: true`, `claude-desktop: true` | Clients recall runs for, keyed by `x-systemprompt-client` name; an unlisted client is off |
| `recall.stale` | `withhold` | `withhold` drops stale facts; `marker` injects them with the stale marker |
| `capture.sources` | `document`, `comment`, `entered` | Pathways captured; an event on any other is ignored before the filters run. `session` is accepted and off by default |
| `capture.session.enabled` | `false` | With `session` in `sources`, the gateway emits `session.turn.closed` per completed turn ([Scaffolding](#scaffolding-inactive-unless-an-extension-opts-in)) |
| `capture.session.max_chars` | `4000` | Characters kept of each of the last user and assistant texts in a session event |
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
| `consumer.source_timeout_ms` | `5000` | Bound on each source read at extraction |
| `consolidation.enabled` | `false` | Runs `brain_consolidate` when a `FactConsolidator` is registered ([Scaffolding](#scaffolding-inactive-unless-an-extension-opts-in)) |
| `consolidation.daily_token_budget` | unset | Tokens the consolidation job may spend per UTC day; unset uses `extraction.daily_token_budget` |

The `brain_per_second` key of `rate_limits` (default `50`) bounds the brain routes.

## HTTP API

All `/api/v1/brain` routes require a signed-in, active user; an inactive account answers `401`. `/api/v1/admin/brain` routes require an admin. Each handler resolves the caller's current roles and the brain service authorizes the operation:

- Every read (listing, a single fact, its timeline, its why-view, a subject's facts) and every dispute applies the same per-fact visibility check recall uses: scope or topic audience, granularity, and whether the caller may read the fact's source as the actor the request acts as. A browser session acts as the person; any other credential acts as an agent. A fact the caller may not see is left out of listings and answers `404` `fact_not_found` when read directly, so its existence does not leak. A `project` fact is visible only when the request names that project (the `project` query parameter).
- A `user`-scoped fact may be governed and entered by its owner only.
- A `project`, `group` or `org` fact may be entered or governed (confirmed, retracted, erased, pinned, unpinned, edited, its disputes resolved) by an admin, or by a holder of the `confirm_role` of a workflow profile that lists the fact's kind. Without such a role the action is refused with `403`.
- Only an admin may list, decide and replay proposals.
- Governing needs visibility first. An admin, or a `confirm_role` holder, may govern or dispute only a fact it can see; a fact it cannot see answers `404` `fact_not_found`, as a read does.

| Method and path | Body or query | Success |
|---|---|---|
| `GET /api/v1/brain/facts` | Query: `kind`, `subject` (`kind:id`), `status` (comma-separated), `limit` (1 to 500, default 100), `project`, `as_of` (RFC 3339) | `200` with an array of facts |
| `POST /api/v1/brain/facts` | `{kind, subject: {kind, id}, fields, scope, topics?, status?, observed_at?}`; `status` is `candidate` (default) or `proposed`; `observed_at` defaults to the insert time | `202` with `{outbox_id}`; the consumer builds the fact. An erased identity answers `409` `fact_tombstoned` |
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
| `PATCH /api/v1/brain/facts/{id}/fields` | `{fields}`, merged onto the stored fields | `200` with the revision; `422` `invalid_fields` when the result does not fit the kind, `403` when it would change the fact's identity |
| `POST /api/v1/brain/disputes/{id}/resolve` | `{fact_id, outcome: upheld or dismissed, note?}` | `204` |
| `GET /api/v1/brain/proposals` | Query: `limit` | `200` with the open proposals |
| `POST /api/v1/brain/proposals/{id}/approve` | None | `204` |
| `POST /api/v1/brain/proposals/{id}/reject` | None | `204` |
| `POST /api/v1/brain/proposals/{id}/replay` | `{since, until}` (RFC 3339) | `200` with the dry-run report, also stored on the proposal |
| `POST /api/v1/admin/brain/simulate` | `{user_id, client, project?, prompt?, reuse?, conversation_id?, as_of?}` | `200` with `{descriptor, skipped?, block?}` |

`scope` is written `{level, key}`, for example `{"level": "project", "key": "sales"}` or `{"level": "org"}`. Unknown query parameters and body fields are refused.

An entered fact is checked against the tombstones before it is queued. Its identity is computed from the declared kind's identity fields, after the subject is resolved through its aliases, by the same function the pipeline uses. An erased identity is refused with `409` `fact_tombstoned` and nothing is queued. The consumer still rejects a tombstoned candidate, which covers a fact erased while its entry was queued.

A field edit is a merge. Each key in `fields` replaces that stored field, a key sent as `null` removes an optional field, and every key left out is kept. The merged fields are checked against the kind (declared, required, material, type, length and enum), the statement is re-rendered from the kind's template, and the edit is refused with `403` when the merged identity fields differ from the stored ones. A partial map therefore edits only what it names. Deciding a proposal only records the decision; nothing a proposal describes applies itself.

### Simulation

`POST /api/v1/admin/brain/simulate` runs a recall for a chosen user, client and project, using `prompt` as the subject hints, and records nothing. Without `project` it resolves the user's default `project` through the subject attribute providers, as the gateway attributes a request that names none, so the simulator and the gateway scope the same recall. With `reuse: true` it runs a same-turn recall and reads the stored block of `conversation_id`, a gateway conversation id; without a `conversation_id`, or when that conversation has no stored block, it answers `not_new_turn`. The `block`, when present, carries the scope, `as_of`, the facts with their revisions, the withheld counts by reason, the tokens and the exact `rendered` text.

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
| `422` | `invalid_fields` | An edit's merged fields do not fit the kind |
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
| `brain_propose` | `kind`, `subject` (`kind:id` string), `fields`, `topics?`, `scope?` (default: the caller's own), `status?` (only `proposed`) | `{outbox_id}`; an erased identity is refused with `tombstoned` |
| `brain_dispute` | `fact_id`, `reason` | `{dispute_id}` |
| `brain_why` | `fact_id`, `project?` | The importance terms (`base`, `subject`, `workflow`, `recency`, `total`) and `withheld` |
| `brain_recall_preview` | `prompt`, `project?`, `as_of?` (RFC 3339) | `injected`, `block` or `skipped`, `withheld` by reason, `tokens`; records nothing |
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

Only a recall that produced a block stores one: an injecting recall, or a recall that missed its deadline and finished afterwards (see [Deadline](#deadline)). A turn whose first recall was skipped for any other reason has no stored block, so every follow-up request of that turn is `not_new_turn` and carries no memory.

### Deadline

A recall that runs past `recall.deadline_ms` injects nothing into that request, which is audited `skipped=deadline`. The engine keeps the same recall running in the background, with no deadline, and records its `deadline` row only when it finishes: when it produced a block, the row carries that block (`rendered_block`, the `injected` refs, `withheld`, `tokens`), stored and never sent. The turn's next request, a tool-result round trip, reuses it through the same-turn path, and its descriptor reads `injected=<n> withheld=<n> tokens=<n> as_of=<time> reused_after_deadline`. The reuse writes an ordinary injecting row, so later requests of the turn reuse that one and carry no suffix. A deadline miss lands exactly one `brain_recalls` row. The simulator applies the same deadline but answers `deadline_exceeded` and runs nothing in the background.

### A new-turn recall

1. **Context.** The principal's user, client, actor and `project` attribution. A workflow profile whose `id` equals the project is selected. `now` is read once and is the only clock every tier, the lifecycle check and the importance score use; `as_of` is `now` unless the request asked for an earlier view. The **held keys** are built once: `org`, `user:<id>`, `project:<attribution>` when there is one, and every audience subject the injected audience resolvers can enumerate for the user (`role:<name>` from core's role resolver, `group:<key>` or any `<dimension>:<value>` from an extension's), rendered in the subject's display form; `everyone` is `org`. A resolver that cannot enumerate contributes nothing, and the subjects it handles stay on the per-fact membership check.
2. **Retrieval.** The subjects the last user message names are resolved once (`recall::tiers::matched`) and shared by the subject and linked tiers: an alias or subject id equal to a word n-gram of the message (one to four words, over the whitespace words with edge punctuation trimmed and over the alphanumeric tokens) scores `1.0`; an alias trigram-similar to the message is reached through the message's *probe words* (tokens of three or more characters no fact carries) and scores its similarity. A subject is named when its best score reaches `alias_min_sim`, the `max_subjects` best are kept, and with `alias_longest_wins` a subject whose every exact alias is a proper substring of another matched subject's exact alias is dropped (a project id no longer names the client inside it). Five core tiers then run concurrently, each from its own index, over the **candidate set** (the facts `brain_working_set` indexes under a held key; kept current on every write), and return ids: *pinned* candidates; the *subject* tier, the candidates about a matched subject (asked kinds first, `subject_limit` rows); the *linked* tier, the candidates one of whose `brain_fact_links` names a matched subject at `score × links_weight`, and at `links_depth: 2` also the candidates about each subject bridged to it through `brain_subject_links` at a further `× links_decay` and the candidates linked to a bridged subject at a further `× links_weight`. No link is followed from a linked fact and no bridge from a bridged subject, so the reach is bounded by the declared depth; a fact about the matched subject itself is never a linked hit, whichever bridge leads back to it (those are the subject tier's rows); and with `links_require_readable_anchor` a matched subject with no fact the reader can read under the tier predicate bridges nothing. The *context* tier returns the request project's candidates by stored importance, asked kinds first. The *full-text* tier runs the plan `recall::search_plan` declares over the weighted search vector: every word of three or more characters, less the `search_stop_words`, is expanded to its synonym group; a group no fact carries is dropped, a group in more than `search_max_df` of the facts only ranks, and of the informative groups, rarest first, a `search_quorum` share (at least one) is required, counted over `search_quorum_over`; a group at or under `search_rare_alone_share` is required alone; a filter that could match more than `search_max_ndoc` rows plans nothing; when the first pass returns nothing and `search_retry_quorum` is declared, a second pass runs at that quorum. Matches rank by cover density (`ts_rank_cd`, length-normalised); the frequencies come from `brain_search_terms`, recounted by the consumer sweep, and before the first sweep every group is optional. Every tier orders by its rank, then stored importance, then fact id, and a fact's age counts in whole days, so two recalls on the same day answer identically. The ids are fused by weighted reciprocal rank (`tiers`, `rrf_k`) and the fused facts are loaded once. Extension retrievers (`FactRetriever`) run beside the core tiers: an id a core tier returned is fused with the retriever's rank; an id none returned is *introduced*, round-robin across retrievers and capped at `extension_max_candidates`, admitted by the tier SQL's own rule and fused as the `extension` tier, so the lifecycle and visibility checks run over it as over any hit. A zero `tiers.extension` weight disables introduction. An extension rank is never a signal (step 4).
3. **Lifecycle.** The tier SQL admits a candidate only as of `as_of`: observed by then (`observed_at`), inside its validity window then, not yet due for review then; `confirmed`, `proposed` only for its owner, `stale` only when `recall.stale` is `marker`. The candidates it keeps out are counted in one grouped query, concurrently with the tiers, and reported as withheld: `stale`, `review_due` and `disputed`. `candidate`, `superseded` and `retired` facts are not candidates and are not counted.
4. **Visibility, then the signal.** Each remaining fact passes `(scope allows or topic audience contains) and granularity allows and source allows`, as described in [Concepts](#concepts). A role or dimension audience is answered from the held keys; only a subject handled by a resolver that could not enumerate is asked per fact, and each distinct membership and source question is asked once. The **recall signal** (`recall::signal`) is then read from the facts the reader can see: the recall *names a subject* when the subject tier returned a visible fact (a subject whose every fact is hidden names nothing, and a fact the linked tier reached through a hidden subject's bridges names nothing); it *has a full-text hit* when that tier returned a visible one; it *asks a kind* when the message carries one of a declared kind's ask words. Before fusion, with `unknown_name_withholds_full_text`, a message that matched no subject and carries a capitalised, non-sentence-initial word of three or more letters that no fact carries (a possessive is the name it owns; stop words excluded) has the full-text tier's ids emptied, the ones no other tier returned are counted as `unknown_name`, and an asked kind is then no signal, so nothing opens the context for a name the brain does not know.
5. **Reranker.** The first registered `Reranker` (an extension seam; core registers none) receives every visible fact with its fused score, the tiers that found it, its kind and statement, with the message's hints and asked kinds, and returns an ordering of a subset of those ids, under `reranker_timeout_ms`. The ids it returns fill first in that order, the ids it omits follow in their fused order, an id it never received is ignored, and an empty result leaves the order unchanged. An error or a timeout is logged with the reranker's name and leaves the fused order untouched. The receipt records which reranker ran and the rank it gave each injected fact (`reranked`).
6. **Fill.** An extension injection policy may choose the facts. Otherwise `usage_weight × (cited_rate − ignored_rate)` is added to each fact's fused score, a fact under `min_fused_score` is withheld as `low_score`, and the facts are taken in `fill_order` (descending fused score, then in-context importance, then fact id; `importance` puts importance first), or in the reranker's order when one ran, while their kind is under its quota (`quota`), the count is under `max_facts` (`max_facts`), and the estimated tokens (a quarter of the characters, rounded up) fit the budget (`budget`). When a subject is named, `linked_share` of `max_facts` is held for linked hits, and facts the context tier alone found may take at most `context_share` of `max_facts`; one past that is `context_cap`. With `context_requires_signal` and no signal (no named subject, no asked kind, and no full-text hit or `full_text_opens_context: false`) that cap is zero: every context-only fact is withheld as `context_cap` and every fact only an extension retriever found as `no_signal`, while a fact another core tier also found is never touched.

The block is appended after the client's system prompt:

```text
<team_memory scope="sales" as_of="2026-10-30T10:00:00Z">
- [<fact id> · doc:<document id>@<revision id>] Acme's primary contact is Jane Doe as sponsor
</team_memory>
```

The scope attribute is the project, or `org` when the request carries none; `as_of` is the instant the facts were read at. The withheld reasons are `stale`, `review_due` and `disputed` from the lifecycle count; `scope`, `source` and `agent_capped` from visibility; `unknown_name` from the name rule; `low_score`, `context_cap`, `no_signal`, `quota`, `max_facts` and `budget` from the fill; `unavailable` is reserved for a source check that fails. The count cut and the token cut are told apart: a fact past `max_facts` is `max_facts` even when it would not have fit either.

An `as_of` recall (`RecallRequest::as_of`, surfaced on the simulator and the preview) reads the facts as they stood then: every tier admits only facts observed by `as_of` and valid at it, the lifecycle count uses it, and the block's `as_of` is the requested instant. It is never later than `now`.

### Audit

`ai_requests.memory_recall` records one line per request: `injected=<n> withheld=<n> tokens=<n> as_of=<time>` or `skipped=<reason>`. It is `NULL` for requests that are not gateway requests. Every recall that reaches the brain writes a `brain_recalls` row: an injection, a same-turn reuse, and the skips `not_new_turn`, `no_facts`, `deadline` and `unavailable`, as the table in [When recall runs](#when-recall-runs) shows. A same-turn reuse writes its own row, and its `memory_recall` reads `injected=<n> withheld=0 tokens=0`. Each row carries the request, conversation, session and context ids, the client, project and selected workflow, the injected `{id, revision}` list, the withheld counts, the tokens and the rendered block, and `reranked` (migration `009_recall_reranker`): the reranker that ran and the rank it gave each injected fact, `NULL` when none ran.

### Outcome signals

| Signal | Recorded when |
|---|---|
| `cited_in_response` | The assistant's response, buffered or assembled from the stream, names an injected fact id as a whole token |
| `user_corrected_fact` | A person disputes, retracts or edits a fact, against every recall that injected it within `lifecycle.correction_window_days` |

Extensions add their own signals through the `OutcomeSignal` seam. Each outcome is unique per recall, fact and signal.

The `brain_*` series on `/metrics` are registered on first use, so they appear only after the brain's first activity in the process.

## Consumer, jobs and health

### The consumer

Each round claims up to `consumer.batch` events on one transaction and commits once. Each event runs inside its own savepoint and its outbox row is acknowledged on the round's transaction, so a bad event rolls back only itself. One commit per round instead of one per event is what lets a larger batch drain faster: the round pays one WAL flush.

- An event that cannot succeed (it does not decode, its version is one the build does not read, or a stage refuses it for good) is recorded as a failure and acknowledged.
- An event that fails for a reason that may pass (storage, a seam call, a timeout, a spent token budget) is rolled back to its savepoint and its attempt is recorded. It stays unacknowledged while the round goes on, and the consumer holds it back from later claims for `poll_secs` before retrying it. Its fifth failure is recorded as abandoned and acknowledged.
- More than three transient failures in one round end the round early, since that many points at an outage. The events it did not reach are released at commit and the consumer waits before the next round.

A failed acknowledgement or commit, or a crash, rolls back the whole round, and every event in it is claimed again. An event's result row is claimed on the same transaction as its facts, so a lost round leaves no result behind and each event is still processed at most once per pipeline version. Facts and their notifications commit or roll back together. A lost round does repeat its extraction calls, so their tokens are spent again.

When nothing is pending the consumer listens on the durable facts channel, with `poll_secs` as a backstop. A lost listener reconnects with exponential backoff from one to thirty seconds. Every `sweep_secs` it sweeps the store under a transaction-scoped advisory lock, so one replica sweeps at a time, and publishes the store gauges. A round that wrote, staled or disputed facts also refreshes `brain_facts{status}` and `brain_disputes_open`, at most once every five seconds per replica, so the gauges follow captures without waiting for the sweep.

`GET /health/detail` reports the consumer under `checks.brain` once it has started:

```json
{"status": "healthy", "state": "listening", "pending": 0, "oldest_seconds": null, "last_round": "2026-11-02T09:00:00Z"}
```

`state` is `idle`, `draining`, `listening` or `reconnecting`. `pending` is the number of outbox events waiting for the consumer, held-back retries included, and `oldest_seconds` the age of the oldest one. `last_round` is when the consumer last claimed. A backlog that cannot be read reports the entry, and the overall status, as `degraded`.

For each event the pipeline normalises it, claims it once per pipeline version, stales the facts its source change replaces, runs the filters, extracts candidates, and for each candidate applies the rules, validates, deduplicates, routes, sets its status, persists and notifies. A new fact's notification commits or rolls back with the fact: `brain.fact.proposed`, `brain.fact.disputed` and `brain.interest.alert`, each with the fact's source as its subject, so recipients are gated by the source's visibility.

### The brain service account

Extraction calls models as the `brain` service account (`brain@service.invalid`), ensured on first start and created by the system admin. Its calls go through the in-process gateway client with an internal origin, a gateway session minted in-process and a `purpose` attribution, so they are metered, guarded, audited and failed over like any other gateway request. Its limits are the gateway policy's quotas on that account. Before each extraction call the consumer checks the day's spent tokens against `extraction.daily_token_budget`.

### Jobs

| Job | Schedule | Work |
|---|---|---|
| `brain_review` | Every 30 minutes | Marks facts past their review date as review-due, 500 at a time, one revision each, and sends `brain.fact.review_due` to whoever confirmed the fact, else its owner, else the `brain-review` stream |
| `brain_retention` | Every 10 minutes | See [Retention](#retention) |
| `brain_usage` | Daily at 02:45 | Sets every fact's `usage_cited_rate` and `usage_ignored_rate` from the recalls and citations of the last `results_retention_days`; see [Usage feeds importance](#usage-feeds-importance) |
| `brain_analysis` | Daily at 03:15 | Runs the core analyses and then every extension analysis over the last day, storing each draft no open proposal already carries |
| `brain_consolidate` | Daily at 03:45 | Scaffolding: offers each kind's facts to a registered `FactConsolidator` and enters its answers as `proposed` facts; inactive by default ([Scaffolding](#scaffolding-inactive-unless-an-extension-opts-in)) |

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
| `brain_working_set_rebuilds_total` | Counter | None; the sweep found the working set's row count drifted and rebuilt it |
| `brain_batch_seconds` | Histogram | None; one sample per drain round |
| `brain_round_events` | Histogram | None; events claimed per drain round |
| `brain_facts` | Gauge | `status` |
| `brain_disputes_open` | Gauge | None |
| `brain_extraction_tokens_today` | Gauge | None |
| `brain_usage_updated_total` | Counter | None; facts whose usage rates the `brain_usage` job wrote |
| `brain_consolidations_total` | Counter | `outcome`: `inactive`, `budget_exhausted`, `entered`, `failed` |

### Report views

- `v_brain_kind_health`: per kind, facts by status, facts past review, injections, citations, corrections, the cited and corrected rates (`NULL` for a kind never injected) and open disputes.
- `v_brain_workflow_recall`: per project and client, with the selected `workflow_id`, recalls run and injecting, facts injected and withheld by reason (including `withheld_max_facts` and `withheld_disputed`), mean tokens per injecting recall, recalls skipped by deadline, unavailability or an empty result, and `late_blocks`, the deadline rows that carry a block produced after the deadline. The injected and withheld sums count injecting rows only.

## Not yet active

These settings and declarations are accepted and validated, and have no effect in this release:

- **The session pathway.** Off by default; see [Scaffolding](#scaffolding-inactive-unless-an-extension-opts-in). Even when switched on, nothing is extracted until an extension registers a filter that gathers session events.
- **`auto_confirm.document: project_policy`.** No project policy source exists yet, so it behaves as `never`: document facts wait for a person to confirm them.
- **`extraction.provider`.** Only `extraction.model` selects the extraction model; the provider is resolved by the gateway from the model.
- **`filters.yaml`.** The document is stored with the other declarations but the pipeline does not read it. Which events are gathered is decided only by registered `EventFilter` seams.
- **Interest fields.** `audience`, `granularity`, and the `memory` and `digest` delivery values are stored and have no effect; only `alert` delivery notifies.

<!-- usage (R6) -->
## Usage feeds importance

The in-context score's usage term is `cited_rate × cited_rate_weight − ignored_rate × ignored_penalty`, with `importance.yaml` defaults of `0.2` and `0.1`. The rates are stored on the fact (`brain_facts.usage_cited_rate`, `usage_ignored_rate`, `usage_updated_at`) and written once a day by `brain_usage`, deterministically and with no model call:

- `injected` is the number of `brain_recalls` rows in the last `lifecycle.results_retention_days` whose injected list names the fact, that were not skipped and that spent tokens, so a same-turn reuse is not a second injection;
- `cited` is the number of `cited_in_response` outcomes on those recalls, and `ignored` is `injected − cited`;
- each rate is Laplace-smoothed, `(x + 1) / (injected + 2)`, so a fact never injected sits at `0.5` on both and usage only separates facts that were actually shown.

The write sets the three usage columns and nothing else: no revision is recorded and `updated_at` does not move, so recency is unaffected. It is the one exception to the rule that `FactRepository::apply` alone changes a stored fact. The why-view reports the term as `usage`.
<!-- end usage (R6) -->

<!-- scaffolding (R8) -->
## Scaffolding: inactive unless an extension opts in

Two pieces ship switched off. Core makes no model call through either; both stay inert until an extension registers the seam **and** the profile turns them on.

**The session pathway.** With `session` in `capture.sources` and `capture.session.enabled: true`, the gateway writes a `session.turn.closed` brain event after each completed turn (buffered and streamed alike) that has a session: the user and session, conversation and context ids, the request id, the last user text and the assistant text bounded to `capture.session.max_chars`, the model and the client. Internal requests, the brain's own calls among them, are never captured. The event's source is `session`, its pathway weight `medium`. Core registers no filter for it, so with nothing registered every session event is recorded as `no_filter` and `brain_extractions` stays at zero; an extension gathers session events with `register_brain_filter!` and reads them with an extractor as it would any other pathway.

**Consolidation.** A `FactConsolidator` (`register_brain_consolidator!`) is offered, once a day, each declared kind it `handles`: the kind's confirmed and proposed facts grouped by subject (groups of two or more), the brain model client and the tokens left in the day's budget (`consolidation.daily_token_budget`, else `extraction.daily_token_budget`, less the extraction tokens already spent that day). Whether it calls the model is its own decision; every call it makes goes through the gateway as the `brain` service account with `purpose=brain.consolidate`. Each candidate it answers takes the entered pathway as a `proposed` fact of the kind it names, entered by the job's actor with `via_agent` set to the consolidator's name, so the consumer validates, deduplicates and routes it like a person-entered fact and a person confirms it. A candidate without a scope inherits the scope of the facts it was built from. Core declares no kind for consolidated facts and registers no consolidator. The job runs only when `consolidation.enabled` is `true`, a consolidator is registered, and the job context carries the brain model client (attached when the API server starts the consumer) and the brain sink; otherwise it succeeds with zero items and logs `brain consolidation inactive: <reason>` once per process.
<!-- end scaffolding (R8) -->

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
| `FactRetriever` | `register_brain_retriever!` | Per new-turn recall, beside the core tiers | Every retriever's ids are fused as the `extension` tier; the ids no core tier returned are introduced, capped at `extension_max_candidates`, and never a signal. A failure contributes nothing. Core registers none |
| `Reranker` | `register_brain_reranker!` | Per new-turn recall, after the visibility pass and before the fill, over the visible facts only | The first registered runs under `reranker_timeout_ms`; the rest are held. An error, a timeout or an empty result leaves the fused order. Core registers none |
| `InjectionPolicy` | `register_brain_injection_policy!` | Choosing which visible facts fill the budget | The first non-`None` plan wins; core's quota-and-budget fill is the fallback |
| `OutcomeSignal` | `register_brain_outcome_signal!` | After a response to a request with injected facts | Every signal runs after core's citation scan |
| `BrainAnalysis` | `register_brain_analysis!` | The daily `brain_analysis` job | Every analysis runs after the three core analyses |
| `FactConsolidator` | `register_brain_consolidator!` | The daily `brain_consolidate` job, per kind it `handles` | The first that handles the kind wins. Core registers none; scaffolding, see above |

Core registers no event filter, so with no extension filter registered no document or comment event is gathered and nothing is extracted. A person- or agent-entered fact bypasses the filters. Recall's source check uses core's document `SourceAccess`, which reads a document source as readable when the reader holds any level on the live document and hides every other subject kind.
