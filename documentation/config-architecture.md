# Configuration architecture: credentials, providers, sync and identity

Status: proposal, 2026-10-22. Scope: core, plus the two installs that consume it (`systemprompt-dynapps`, `systemprompt-astound`).

What prompted it:
- Victor's missing OpenCode Go models, Dynapps 0.72.0 → 0.72.1.
- The `credential_from` patch that shipped to fix them.

Evidence comes from four read-only audits of the core, dynapps and astound trees. `file:line` refs are as of core `ce4512196`.

---

## 0. Constraints (non-negotiable)

1. **Fix forward. No backwards compatibility and no dual paths.**
   - Each phase replaces the old shape in one release.
   - There are no legacy loaders, translation layers, deprecation windows or compatibility flags.
   - Core, dynapps and astound are migrated together in that release.
2. **A clear, mechanical migration path:**
   - **Config:** a one-shot `systemprompt config migrate` rewrites a `services/` tree and profiles to the new shape. It is idempotent, and its output is committed by the install.
   - **Data:** a numbered schema migration moves existing data to the new keys in the same release:
     - `user_provider_credentials.provider` becomes the credential id;
     - route ids and `access_control_*` rows are rewritten to declared ids;
     - `ai_requests` gains `credential` and `endpoint`.
     Nothing that a member stored is lost.
   - **Release:** the new binary refuses the old shape with an error that names `config migrate`. It never half-loads.
3. **Multi-node and no write access to source in production.**
   - Nodes run from an immutable image.
   - The console must work and persist without touching code.
   - Code-to-database (apply) and database-to-code (export) are explicit, auditable steps.
4. **One concept, one place.** A reviewer reading `services/` should find each fact (a model, a key, a default, a rule) stated once, with nothing encoded in naming conventions, rule order or prose.

---

## 1. Why the current model produces workarounds

`ProviderEntry` (`crates/shared/manifest/src/services/providers/mod.rs:113-156`) binds five independent concerns to one name:

| Concern | Field today | Should be |
|---|---|---|
| Who authenticates and who pays | `api_key_secret`, `credential`, `credential_from` | **Credential** |
| The vendor account (headers, user agent, governance) | `name`, `user_agent`, `extra_headers`, `forward_headers`, `governance` | **Provider** |
| How and where to call it | `wire`, `endpoint`, `accepted_betas` | **Endpoint** |
| What a model is | `models[]` (capabilities, limits, pricing) | **Model** |
| Which models are served where, and failover | split between `models[]` membership and gateway `routes` | **Deployment** |

The provider name is also used as the database key (`user_provider_credentials`), the audit dimension (`ai_requests.provider`), the quota subject and the route target. Every place that needs one concern without the others has grown a workaround.

### 1.1 Workarounds this forces (from the audits)

**OpenCode Go is one subscription split into two providers.** It has one key but two wires.
- `dynapps services/ai/providers.yaml:170` (opencode-go) and `:330` (opencode-go-anthropic).
- Each entry repeats the same `user_agent` and `forward_headers`.
- The fix shipped in 0.72.1 is `credential_from`.

**The same model sits under several ids.** Ids must be unique across the whole catalog (`validate.rs:69-77`).
- Astound declares Claude three times: `anthropic-claude-*` hidden plus `upstream_model`, `vertex-anthropic` with the bare ids, and `vertex-eu-claude-*` hidden aliases.
  - `astound providers.yaml:8,415,643`.
  - Failover links them back through `find_served_model` matching on `upstream_model` (`mod.rs:204-211`).
- The same DeepSeek V4 model is listed under both `deepseek` and `go-`, and the two copies disagree on cache pricing.
  - `dynapps providers.yaml:126,148` vs `:242,264`.

**Regions and credential types become providers.**
- Astound: one GCP service account appears as five entries.
  - `vertex`, `vertex-anthropic`, `vertex-anthropic-eu`, `vertex-maas`, `vertex-maas-us-central1`, all with `api_key_secret: vertex`.
  - Comment at `astound providers.yaml:1106`: "an endpoint is per provider, so it needs a provider of its own".
- The EU price premium is typed in by hand (+10%, `:655`).

**Wire choice is encoded in route order.** Routing ignores the catalog: with no declared route, everything goes to `default_provider` (`gateway/config/runtime.rs:84-139`), so installs restate the catalog as ordered `model_pattern` globs.
- `dynapps gateway.yaml:23-44`: "First match wins: these two must stay above the go-* catch-all".
- `astound gateway.yaml:116-122`: the same pattern for regions.
- A model whose name doesn't fit the glob silently reaches the wrong wire.

**Routes that route nothing.**
- `astound gateway.yaml:56-75`: four `by_scope` chains identical to the default chain. They exist only to get `route_match: scope:project=…` into the audit row.

**Betas copied by hand.**
- `astound providers.yaml:460-463` and `:666-669` hold identical `accepted_betas` lists ("mirrors vertex-anthropic").
- Betas belong to the Vertex-Anthropic endpoint, not to a provider entry.

**`surface` is overloaded.**
- It means "which client family lists this", not "the wire" (`surface_for` has no production caller).
- It doubles as "not advertised" (`backend`).
- The loader rewrites it to `backend` when a secret is missing (`config_loader/mod.rs:262-281`).
- Installs use it as a trick to hide DeepSeek from Claude clients: `wire: anthropic, surface: openai`, `dynapps providers.yaml:111-119`.

**Defaults stated three times.**
- `ai.default_provider` and `ai.providers.<x>.default_model` (`services/ai/config.rs:21`).
- `gateway.default_provider` and `gateway.default_model` (the latter unvalidated, `bridge.rs:174`).
- Claude Code's `[1m]` context marker hand-typed into the gateway default: `claude-sonnet-5-5[1m]` (dynapps) and `claude-sonnet-5[1m]` (astound, a different generation).
- `ai.providers.<x>.enabled` is honoured only by `AiService` (`service.rs:143`). The gateway, bridge profile and route catalog ignore it.

**Real bugs caused by the coupling.**
- **Catalog CLI downgrades personal providers.** `admin config catalog` upsert (`infra/config/src/services/provider_catalog.rs:23-77`) rewrites a `credential: user` entry to Platform and drops its `user_agent`, headers and betas.
- **Failover can cross subscriptions.** The guard only compares `is_user_credential()` between hops (`app/gateway/src/service/resolve.rs:204-216`), so failover can move between two different member subscriptions.
- **Only one endpoint of a shared key is verified.** Key verification probes only the owning entry's endpoint (`personal_keys.rs:115-191`); sibling endpoints are never checked.
- **Token cache keyed per entry, not per credential.** The key is `user:{uid}:{entry}` (`target.rs:51`), so entries sharing a credential mint separately.

---

## 2. Target model: five objects, each with one job

```yaml
# services/ai/credentials.yaml
credentials:
  anthropic-key: { kind: platform, secret: anthropic }
  vertex-sa:     { kind: platform, secret: vertex }          # API key or service account, detected from the secret
  opencode-go:   { kind: personal, label: "OpenCode Go" }    # each member stores their own; shown once in Settings

# services/ai/providers.yaml
providers:
  opencode-go:
    credential: opencode-go
    user_agent: "systemprompt-gateway (opencode-go; {install})"
    forward_headers: [x-opencode-session]
    governance: { european: false, no_retain: false }
    endpoints:
      chat:     { wire: openai-chat, url: https://opencode.ai/inference/go/openai/v1 }
      messages: { wire: anthropic,   url: https://opencode.ai/inference/go/anthropic/v1, accepted_betas: [] }
  vertex:
    credential: vertex-sa
    endpoints:
      claude-global: { wire: anthropic, url: "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/…", accepted_betas: [...] }
      claude-eu:     { wire: anthropic, url: "…/locations/europe-west4/…", accepted_betas: inherit:claude-global, price_multiplier: 1.10 }
      gemini:        { wire: gemini, url: … }
      maas-us:       { wire: openai-chat, url: "…/locations/us-central1/…" }

# services/ai/models.yaml: a model is defined once
models:
  claude-sonnet-5-5: { capabilities: { vision: true }, limits: { context_window: 1000000 }, pricing: { … } }
  go-minimax-m3:     { … }
  deepseek-v4-pro:   { … }

# services/ai/deployments.yaml: which model is served where; order is failover
deployments:
  claude-sonnet-5-5: [vertex/claude-global, vertex/claude-eu]
  go-minimax-m3:     [opencode-go/messages]
  go-kimi-k3:        [opencode-go/chat]
  deepseek-v4-pro:   [deepseek/messages, { endpoint: opencode-go/chat, upstream_model: deepseek-v4-pro, pricing: { cache_read_per_million: 0.132 } }]

# services/ai/gateway.yaml: routes only for real policy
gateway:
  default_model: claude-sonnet-5-5          # the one default; "[1m]" is derived from limits.context_window
  routes:
    - id: project-commerce-eu-only         # declared, stable, human-readable
      when: { scope: { project: commerce } }
      models: claude-*
      deployments: [vertex/claude-eu]
```

### 2.1 Rules

- **Credential.** The unit of authentication, billing (`billing_source`) and the Settings UI.
  - Personal credentials are stored in `user_provider_credentials` under the credential id.
  - The token cache, failover billing guard and key verification all key on the credential.
  - Verification probes every endpoint the credential serves.
- **Provider.** The vendor account: headers, user agent and governance. It owns endpoints and has one credential. `{install}` and similar placeholders remove copied strings.
- **Endpoint.** One wire plus one URL, with endpoint-scoped `accepted_betas`, region, `price_multiplier` and hosting.
  - `UpstreamCall` already takes the wire as an argument (`crates/domain/ai/src/services/upstream/call.rs:92-185`), so the per-request wire comes from the endpoint, not the provider.
- **Model.** Identity, capabilities, limits and base price, stated once.
  - Ids are globally unique, so no `go-`, `vertex-eu-` or `anthropic-` prefixes are needed to dodge clashes.
  - `upstream_model` defaults to the id.
- **Deployment.** Model → ordered endpoints, with optional per-deployment `upstream_model` and price overrides. This *is* routing for the common case. Dispatch resolves `model → deployment → endpoint → provider → credential` with no glob matching.
- **Route.** Policy only: scope, region, pinning, aliasing. It has a declared, stable id.
  - Generated `slug-fnv1a6` ids stay valid for legacy routes (see §5).
  - The audit label is a separate field from the id.
- **Visibility.** `clients: [claude, openai, gemini]` on a model or deployment replaces `surface`.
  - "Not advertised" is `clients: []`.
  - "Has no credential" is health state, never a rewritten field.
- **Defaults.** One `gateway.default_model`.
  - `ai.default_provider`, `ai.providers` and the gateway default provider are derived from it.
  - `enabled: false` on a provider applies everywhere.
- **Context variants.** `[1m]` is a client presentation of `limits.context_window >= 1M`, computed by the profile builder and stripped by the gateway (as today). It is never written in config.

---

## 3. Sync: one rule for every plane, safe for multiple nodes

Today there are three writers with three conflict policies on the same rows:

| Writer | Location | Conflict policy |
|---|---|---|
| Core composed-bundle reconcile | `crates/infra/governance/src/authz/reconcile.rs:93-136` | inserts `gateway_route` rows with `default_included=false`, prunes by set difference |
| Dynapps route catalog | `admin-core/repositories/sync/route_catalog.rs` | `ON CONFLICT DO NOTHING` |
| Core `roles.yaml` ingest | `repository/ingestion.rs:39-53` | `ON CONFLICT DO UPDATE` (last rule wins) |

They also run in different directions:
- Gateway policies: the file overwrites the DB on every boot.
- Access control: the file seeds the DB once, then only reports drift.
- Core config planes: file- or DB-authoritative, depending on a profile flag (`profile/config_planes.rs`).

Boot order decides the result. This is why the go-* routes were born closed in production.

### 3.1 The rule

1. **At runtime, every plane is read from the database:**
   - access control;
   - gateway policies (quota, safety);
   - governance mode;
   - routes and deployments;
   - groups.
   All nodes read the same rows. Each node caches them and refreshes on a `config_version` bump, signalled through Postgres `LISTEN/NOTIFY` with a short poll as fallback. No restart is needed.
2. **Console edits write to the database.**
   - Each edit records `changed_by`, `changed_at` and a `reason`; the reason is required on shared bands.
   - The edit takes effect cluster-wide immediately. It never touches code.
3. **Applying code to the database is explicit.** It runs on deploy (`systemprompt config apply`, called by the deploy pipeline) or from `/admin/sync`. It never runs as a side effect of boot.
   - **Single applier:** a Postgres advisory lock, as migrations use, so a rolling deploy cannot race itself.
   - **Three-way merge per row.** `sync_state` already stores the hash of the last apply, which is the base.

| Declared (file) changed since last apply | DB changed since last apply | Result |
|---|---|---|
| no | no | no-op |
| yes | no | apply the file (a normal deploy) |
| no | yes | keep the console edit and report it as drift |
| yes | yes | **conflict**: nothing is applied for that row; `/admin/sync` shows both sides and needs a decision |

4. **Exporting database changes to code is a developer action.**
   - `/admin/sync` → Export, or `systemprompt cloud sync export`, produces a YAML patch for a human to commit.
   - The running system never writes source.
   - When the exported change is deployed, the hashes match and the drift clears.
5. **Each row has exactly one writer per origin.**
   - Bundle reconcile and route catalogs create entity rows only if missing (`ON CONFLICT DO NOTHING`) and **never** set `default_included`. Only the access plane writes it.
   - `source` uses one closed vocabulary: `code:<file>`, `bundle:<name>`, `console:<user>`, `migration`.
6. **One access-control dialect.** Core reads one file, `services/access-control/rules.yaml`, in the dynapps `entities:` dialect (default open|closed, allow by role/group/project/connector, a required `why`).
   - `roles.yaml` is deleted; `config migrate` converts it.
   - The deny message (`decision.rs:73,82`) names the file that actually exists.
7. **Rules match by stable keys, not generated ids.**
   - `gateway_route/<declared-id>` for declared routes.
   - `model/<glob>` and `provider/<name>` for entitlement. These are evaluated at decision time against the live catalog, not expanded once at ingest (`ingestion/resolve.rs:77-88`), so a route added later is covered without a re-ingest.

### 3.2 What this fixes
- **The quota warn/enforce mismatch.** Today a console change to `enforce` is silently reset to the file's `warn` at the next boot or "Overwrite from code". Under this rule it is visible drift and persists until someone decides.
- **go-* routes born closed.** The entity writers no longer touch `default_included`.
- **Operational fit.** It works on multi-node Fly or Kubernetes, with an immutable image and no source access in production.

---

## 4. Identity: one principal model

- **PATs carry no roles.**
  - `access_scope()` returns `Unknown`, and `authz_attributes()` returns empty roles, groups and projects for API keys (`crates/entry/api/src/routes/gateway/messages/auth.rs:75-104`).
  - The same person is therefore allowed through the bridge (JWT with roles) and refused with a PAT (403 on role-gated routes).
  - **Target:** every credential type (bridge JWT, PAT, hook token, OAuth) resolves to the same `Principal { user, roles, groups, projects, client }`. Roles and groups are loaded from the user record at auth time and cached briefly, not snapshotted into tokens.
  - PAT `scopes` narrow that principal; they are not a parallel authorization model.
- **Session id.**
  - The gateway requires `x-session-id`, but OpenCode 1.19 overwrites it with its own `ses_…`.
  - **Target:** the session id is bound to the credential. The client's header is recorded as `client_session` and never rejected.
- **Organisation id.** Today it is derived in three places:
  - the gateway's `canonicalize_org_uuid(tenant_id)` (`bridge.rs:160-212`);
  - the bridge override `deployment_organization_uuid`;
  - the bridge pin `cowork.session_org_dir`, which ranks below the org match.
  - **Target:** the gateway returns one canonical `organization_uuid`. The bridge never derives it. An explicit local pin always wins.

---

## 5. The bridge consumes facts and never re-derives them

- **The profile is always fetched as the user.**
  - Done in 0.72.1: the caller passes the bearer.
  - **Target:** the profile is never cached anonymously. The cache is keyed by `(gateway, user)` (`profile_cache.rs:44-75`), and a stale bearer returns 401, not a silent anonymous profile (`bridge.rs:186-194`).
- **The profile's model list uses the same check as dispatch:** servable, entitled (access control) and route-resolved (`resolve_route`, not `find_route`). Today `bridge_profile.rs:108-140` ignores entitlement and `hidden_requires_route`.
- **Model visibility per host comes from the model's `clients`.** It does not come from the hardcoded `claude-*` / not `claude-fable-*` predicate (`bin/bridge/src/gateway/model_view.rs:18-21`).
- **Host model filters have one store,** the server. The local sentinel copy is removed.

---

## 6. Profiles

- **Base plus overlays.**
  - `profiles/base.yaml` holds everything shared.
  - `profiles/<env>.yaml` holds only what differs: URLs, tenant, sources, log level, `allow_registration`.
  - **The evidence:** the dynapps local and production profiles differ only in environment fields, and astound has six near-copies.
- **Derived fields, not repeated ones.**
  - `api_server_url`, `api_external_url`, `jwt_issuer` and CORS come from one `external_url`.
  - The governance authz URL comes from the server address.
  - Paths are relative to one `root`.
- **Bundle sources come from one list** (`deploy/kit/known-kits.json`), selected per environment.
  - Today local and production load different bundles: astound local `sfnext,india` against production `sfnext,sfcore`. Local therefore cannot reproduce production.
- **Gates.**
  - No absolute temp paths in committed profiles (astound `kit-check/profile.yaml:38`).
  - `tenant_id` lives in one place (it is duplicated in `profile/info.rs:18` and `profile/cloud.rs:13`).

---

## 7. Shared baseline

Core ships defaults that installs override only where they differ:
- the gateway header block;
- `gateway_route/* default: open` (repeated verbatim in astound and dynapps);
- quota-mode semantics;
- per-host betas (Vertex-Anthropic);
- calendar quota windows;
- capability defaults (only `vision` varies across today's catalogs).

Model `capabilities` become provider-level defaults with per-model overrides, ending the ten-field block repeated on every model.

---

## 8. Plan

Each phase is one release:
- core lands the new shape and deletes the old;
- `config migrate` and a data migration ship in the same release;
- dynapps and astound adopt it in their release commit.

There are no compatibility layers. §9 is the proof that each cut-over preserves behaviour.

### Phase 0: bugs and dead weight (0.72.x; no schema change)
1. **Catalog CLI:** preserve `credential`, `credential_from`, `user_agent`, `forward_headers`, `extra_headers` and `accepted_betas` (`provider_catalog.rs:23-77`), with a regression test.
2. **Failover billing guard:** compare `credential_owner()`, not `is_user_credential()` (`resolve.rs:204-216`).
3. **Personal-key verification:** probe every entry that shares the credential (`personal_keys.rs:115-191`). Key the token cache by credential owner (`target.rs:51`).
4. **Entity writers:** core bundle reconcile and the dynapps route catalog never set `default_included` on conflict, and new rows take the access plane's declared default. This fixes go-* routes being born closed.
5. **Deny message:** names the access-control file that exists.
6. **Installs:**
   - astound drops its month-window job (core has `calendar: month`), the stale `claude-native-tools` comments, the retired Vertex routes and models, and the scratch path in `kit-check`;
   - dynapps `config.yaml` drops `ai.providers` entries that only restate the catalog.

### Phase 1: identity and bridge facts (0.73)
1. PAT principals resolve roles, groups and projects from the user record, and scopes narrow them. This is a contract change, so the core test `gateway_messages_auth.rs` asserting `roles []` is updated.
2. The bridge profile model list runs the same servable + entitled + route-resolved check as dispatch. The cache is keyed by user; a stale bearer gives 401.
3. One canonical `organization_uuid` comes from the gateway, and the bridge stops deriving it.
4. Session binding: `x-session-id` from the credential, with the client header recorded.

### Phase 2: the catalog model (0.73, breaking)
1. **Core replaces `ProviderEntry`** with `credentials`, `providers.endpoints`, `models`, `deployments`, `clients`, and routes over deployments. The old fields are deleted:
   - `ProviderEntry.wire`, `.endpoint`, `.surface`, `.api_key_secret`, `.credential`, `.credential_from`;
   - `ai.default_provider`, `ai.providers`;
   - `model_pattern`-to-provider routes.
2. **Dispatch** resolves `model → deployment → endpoint`. `UpstreamTarget` takes its wire, URL and betas from the endpoint and its credential from the provider.
   - Every consumer (`resolve.rs`, `stages/*`, pricing, `bridge_profile.rs`, `personal_keys.rs`, vertex discovery, `AiService`, `provider_catalog.rs`) moves to the new types in the same change.
3. **`config migrate`** rewrites the legacy shape:
   - entry → provider + endpoint;
   - `api_key_secret` → a platform credential;
   - `credential: user` / `credential_from` → one personal credential;
   - sibling entries sharing a vendor and credential → one provider with several endpoints;
   - models listed under several entries (same `upstream_model`) → one model with several deployments;
   - `surface` → `clients`;
   - ordered `model_pattern` routes → deployments (pure routing) or policy routes (`when` / scope);
   - `[1m]` stripped from defaults.
   It reports anything it cannot decide (e.g. two price cards for one model) and stops; it never guesses.
4. **Data migration:**
   - `user_provider_credentials.provider` → the credential id, mapped from the entry name.
   - `ai_requests` gains `credential` and `endpoint`, backfilled from the entry name. `provider` keeps its historical values, so reports stay comparable.
   - Quota windows are re-keyed to the credential.
5. **Installs, in their 0.73 release commit:**
   - **dynapps:** one `opencode-go` provider with `chat` and `messages` endpoints; DeepSeek V4 as one model with two deployments; the ordered `go-*` routes deleted.
   - **astound:** one `vertex` provider with region endpoints; Claude defined once; the EU premium as `price_multiplier`; no-op `by_scope` chains deleted; betas declared once on the endpoint.
   - Both installs: profiles become base plus overlays.

### Phase 3: sync (0.74, breaking)
1. Add `config_version` with `LISTEN/NOTIFY` cache invalidation, plus `changed_by`, `changed_at` and `reason` on every plane table.
2. Add `systemprompt config apply`: advisory lock, three-way merge against `sync_state`, and conflicts surfaced on `/admin/sync`. The deploy pipeline calls it.
3. **Boot no longer applies anything.**
   - Gateway-policy boot overwrite, bundle-reconcile defaults and seed-if-empty are all deleted.
   - The first `apply` on 0.74 records the current DB as the base, so nothing is overwritten on upgrade.
4. **One access-control dialect,** `rules.yaml` `entities:`. `roles.yaml` is deleted, and `config migrate` converts it.
   - Rules match `gateway_route/<declared-id>`, `model/<glob>` and `provider/<name>` at decision time.
   - A data migration rewrites generated `slug-fnv1a6` route ids to declared ids, together with their rules.
5. Add `systemprompt cloud sync export`, which writes a YAML patch for a developer to commit.

### Phase 4: removal sweep (0.74)
- Delete everything §2–§7 replaces that still exists, together with its tests:
  - `surface_for`;
  - the `Backend` rewrite in the config loader;
  - `find_served_model`'s `upstream_model` back-link;
  - the bridge's `claude-*` Desktop predicate;
  - the local host-filter sentinel;
  - `canonicalize_org_uuid` in the bridge path;
  - profile aliases (`evaluation`, `MOVED_SECTIONS`).
- Gate: a source check that fails if any removed name reappears.

---

## 9. Proof that each cut-over preserves behaviour

1. **Golden migration.**
   - The committed dynapps and astound `services/` trees and core's default catalog at the previous release are the fixtures.
   - `config migrate` runs over each.
   - The resolved runtime catalog after migration is compared with the one the previous release resolved from the original. It must be identical in:
     - models;
     - prices, including the EU multiplier;
     - endpoint and wire per model;
     - credential per model;
     - advertised models per client;
     - the target for every model id.
   - The previous release's resolution is captured once as a JSON snapshot and committed.
2. **Wire replay.** The dynapps harness (`tests/integration/harness`), real CLI captures and the 120-audit production replay run on the migrated trees. Outbound bodies and headers must match byte for byte, except for the intended changes.
3. **Data migration.**
   - Seed a DB at the previous release's schema with personal keys, route ids, rules and `ai_requests`.
   - Run the migration, then assert that every key decrypts and resolves to its credential, every rule still governs the same requests, and reports aggregate the same.
4. **Sync semantics.** For each of the four rows of the §3.1 table, a test runs two concurrent appliers and asserts that exactly one applies.
5. **Schema ladder.** The existing release-baseline rungs migrate forward to each phase's schema.
6. **Principal parity.** The same user via bridge JWT and via PAT gets the same decision on every role-gated route (Phase 1).
7. **Refusal.** A legacy tree given to the new binary fails at load with the `config migrate` hint, and never half-loads.

## 10. Already in 0.72.1 (shipped as patches; superseded by §2 and §5)
- **`ProviderEntry.credential_from`:** one personal key covers sibling entries. It is deleted in Phase 2, and `config migrate` folds it into a personal credential.
- **Bridge profile fetched with the user's bearer** (`fetch_bridge_profile(Option<&BearerToken>)`).
- **Bridge re-mints once on a stale-token 401** (`proxy/forward/retry.rs`, `token_cache::rejection_is_stale`).
- **`oauth: false` on bridge-written OpenCode MCP entries.**
