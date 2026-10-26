# Upgrading to 0.73

How to move a production deployment from an earlier release to 0.73.0. The release changes four things an operator acts on: a profile states its public URL once and lives at `profiles/<env>/profile.yaml`, the AI provider catalog has a new file format, declared configuration reaches the database only through an explicit `config apply`, and the gateway and bridge must be upgraded together. The full list of changes, with a migration note for each, is under `### Breaking` in `CHANGELOG.md` and the per-crate changelogs.

## At a glance

| Area | Action | Required |
|---|---|---|
| Profiles | `systemprompt admin config migrate --profiles <dir>` | Yes. A 0.72 profile is refused at load, so no other command runs until it is migrated |
| AI provider catalog (`services/ai/`) | `systemprompt admin config migrate` | Yes. A 0.72 tree is refused at boot |
| Configuration planes | `systemprompt admin config apply --reason <why>` after every upgrade, and once on a fresh install | Yes. Unapplied planes are not enforced |
| Profile `governance.unapplied_planes` | New: `warn` (default) or `deny` | Choose a value |
| Services bundles | Repack every bundle with the 0.73 CLI | Yes, if you publish bundles |
| Plugin hooks | `hooks.evaluation` becomes `hooks.judge` | Yes. The old key is refused |
| `gateway.catalog_route_id` | Removed. `config migrate` moves the rules it carried | Yes, if you set it |
| Database | 24 forward-only migrations | Yes. Run before serving |
| Stored catalog data | `admin config migrate --data` after the database migrations | Yes |
| Bridge clients | A 0.73 bridge needs a 0.73 gateway, and the reverse | Yes. Roll out together |
| Rust consumers | Process globals are replaced by values owned by the application context | Only if you build on the crates |

## 1. Convert the services tree

### 1.1 Profiles

A profile now states its public URL once, as `server.external_url`. The loader derives the rest from it and from the bind address, and refuses a profile that states any of them:

| Removed key | Derived from |
|---|---|
| `server.api_server_url`, `server.api_external_url` | `server.external_url` |
| `server.use_https` | the scheme of `server.external_url` |
| `server.cors_allowed_origins` | the origin of `server.external_url`, plus `server.extra_cors_origins` |
| `security.jwt_issuer` | `server.external_url` |
| `server.api_internal_url` | `server.host` and `server.port` |
| `governance.authz.hook.url` | `server.host` and `server.port` |

A profile lives at `profiles/<env>/profile.yaml`, optionally layered over a shared `profiles/base.yaml`: maps merge with the overlay winning, and a list in the overlay replaces the base's. The `<name>.secrets.profile.yaml` layout is refused. Relative paths resolve against `paths.root`, which defaults to the profile's directory.

Migrate the profiles before anything else, because no other command runs on a 0.72 profile:

```bash
systemprompt admin config migrate --profiles .systemprompt/profiles --check   # exits non-zero while a change is pending
systemprompt admin config migrate --profiles .systemprompt/profiles
```

The command loads no profile. For each layer it replaces the URL keys with one `server.external_url` in place, so comments stay attached. CORS origins other than the public one become `server.extra_cors_origins`, and a top-level `evaluation:` becomes `judge:`. A `<name>.secrets.profile.yaml` moves to `<name>/profile.yaml` and gains `paths.root: ".."`, so its relative paths resolve where they did. Every rewritten profile is loaded through the 0.73 loader before anything is written, and a second run makes no changes.

The command refuses, and writes nothing, where 0.73 would silently change behaviour:

- `security.jwt_issuer` differs from the public URL. 0.73 issues tokens as `server.external_url`, so every token issued under the old issuer would stop verifying. Decide which URL is public and make both say it.
- `api_server_url` and `api_external_url` disagree, or `use_https` contradicts the URL's scheme.
- `api_internal_url` or the authz webhook URL differs from what `server.host` and `server.port` give.

A legacy profile that moves into a directory with a `base.yaml` starts inheriting the base's keys, and the report says so. `${VAR}` references are carried over verbatim, so run the migration where those variables are set: the verification step resolves them.

### 1.2 The provider catalog

The single `providers.yaml` list of providers with nested models is replaced by five objects in fixed files under `services/ai/`:

| File | Holds |
|---|---|
| `credentials.yaml` | Credentials: `kind: platform` with a secret name, or `kind: personal` for a key each user supplies |
| `providers.yaml` | Providers, each with a credential, capabilities and named `endpoints:` (wire and URL) |
| `models.yaml` | Models: `clients` (`claude`, `openai`, `gemini`), limits and pricing |
| `deployments.yaml` | For each model, the `<provider>/<endpoint>` hops that serve it, in failover order |
| `gateway.yaml` | `default_model` and the policy routes |

Dispatch resolves `model → deployment → endpoint → provider → credential`. Setting `enabled: false` on a provider removes its endpoints from every deployment.

The loader reads these files by name. An `includes:` entry naming a catalog file, or a catalog section in any other file, is refused. A tree in the 0.72 shape fails at boot with a hint to run `systemprompt admin config migrate`.

Removed keys: `ai.default_provider`, `ai.providers`, `gateway.default_provider`, `gateway.catalog_route_id`, `allow_unlisted_models`, `hidden_requires_route`, `model_pattern` routes, route `by_scope` and `fallbacks` chains, route `extra_headers`, and the per-provider `surface` (replaced by each model's `clients`).

### 1.3 Gateway routes

Every route carries a declared `id`. The other route keys are `label`, `models` (globs), `when.scope`, `deployments`, `context_fallbacks` and `alias`.

```yaml
gateway:
  enabled: true
  default_model: claude-sonnet-5        # must not carry a "[1m]" suffix
  routes:
    - id: claude-primary
      label: Claude, primary region
      models: ["claude-*"]
      deployments: [anthropic/api]
```

`gateway.catalog_route_id` is removed. A request that no declared route matches has no route id, and access control decides it on the `model/<id>` and `provider/<name>` entities that match it. `config migrate` moves rules that were addressed to a routing-only route or to the catalog route onto the matching `model/<glob>` entity and records the move in `services/ai/legacy_map.yaml`. `config migrate --data` (step 5 of [the upgrade order](#5-upgrade-order)) applies the move to the stored rules.

An alias route sends the target model's `upstream_model` upstream.

### 1.4 Plugin hooks: `evaluation` is `judge`

A plugin descriptor's hooks block says `judge`. The `evaluation` alias is removed, a tree that still uses it is refused, and `evaluation/` is no longer an allowed services-bundle directory. `config migrate` renames `hooks.evaluation` to `hooks.judge` and keeps comments, and `config migrate --check` reports the rename as pending.

The other retired serde aliases are removed as well:

| Where | Refused | Use |
|---|---|---|
| Plugin hooks block | `evaluation` | `judge` |
| MCP deployment | `type` | `server_type` |
| Agent card | alias | `agentRun` |
| Claude plugin manifest | alias | `mcpServers` |
| `marketplace.json` | aliases | `pluginRoot`, `allowCrossMarketplaceDependenciesOn` |

### 1.5 Access control

Core reads one access-control file, `services/access-control/rules.yaml`, with one `entities:` block per `<kind>/<id>`. `access-control/roles.yaml` is no longer read, and a tree that still carries it is refused. `config migrate` folds it into `rules.yaml`.

A marketplace's `access:` block is no longer ingested at boot or by `services refresh`. `config apply` reads it as the `marketplace/<id>` entity. Declaring the same entity there and in `rules.yaml` is refused.

### 1.6 Host ids

There are seven bridge host ids: `claude-code`, `claude-desktop`, `codex`, `codex-cli`, `opencode`, `opencode-cli` and `hermes`. `codex` and `opencode` name the desktop apps. `codex-cli` and `opencode-cli` name the command-line tools. Update `hosts:` lists, host rules and `enabled_hosts` that mean a CLI. A manifest that names an unknown host id fails to load, and an empty `enabled_hosts` enables no host.

### 1.7 Running `config migrate`

```bash
systemprompt admin config migrate --check   # exits non-zero while a change is pending
systemprompt admin config migrate           # rewrites the tree and writes services/ai/legacy_map.yaml
```

The command writes nothing until the result loads through the real loader. When it meets something it cannot decide, it stops and names the file and keys. A second run makes no changes. Commit the rewritten files and `legacy_map.yaml`, because `--data` reads that file.

To migrate a tree against a database other than the profile's, pass both flags:

```bash
systemprompt admin config migrate --services ./services --database-url <url> --data
```

`--database-url` requires `--services`. Without `--services`, the services directory would come from the profile while the data came from another database, so the command refuses the combination.

## 2. Apply configuration explicitly

### 2.1 `config apply` is mandatory

Boot and the `config_reconcile` job no longer write any configuration plane. They compare each declaration with the database and report drift. A declaration reaches the database only through an explicit apply:

```bash
systemprompt admin config apply --reason "upgrade to 0.73.0"
```

Run it in the deploy pipeline after the database migrations and `config migrate --data`, after every later release, and once on a fresh install, which boot no longer seeds. Pass `--plane <name>` to apply one plane, and `--dry-run` to see what would change.

The apply runs on one replica at a time and merges each entity three ways against the last applied base:

- A row only the file changed is written.
- A row only the console changed is kept and reported as drift.
- A row both changed is recorded as a conflict and left alone, and the command exits non-zero.

The first apply of a plane adopts the database as its base, so nothing written through the console is overwritten on upgrade. `systemprompt cloud sync export` renders each plane's declaration with the console's drift and conflicts folded in, so the result can be committed.

| Plane | Declared in |
|---|---|
| `governance` | `services/governance/config.yaml` |
| `gateway_policies` | `services/gateway/policies.yaml` |
| `access_control` | `services/access-control/rules.yaml` and marketplace `access:` blocks |

The apply records every plane it runs, declared or not. A tree without `governance/config.yaml` runs the warn-only chain, and one without `gateway/policies.yaml` runs the permissive policy.

### 2.2 `governance.unapplied_planes`

Until an apply records a plane, the plane is unapplied. The profile decides what a node does with an unapplied plane:

```yaml
governance:
  unapplied_planes: warn   # or deny
```

| Value | Behaviour |
|---|---|
| `warn` (default) | Requests pass without governance. The node warns at boot, on every governed and gateway request, in `/readyz`, in `/health` (status `degraded` with a `governance` block) and in `cloud doctor` |
| `deny` | Every governed call is refused with `governance plane not applied`, and every gateway request returns 503. The server stays up |

An unknown value is refused when the profile is read. An empty `access_control` plane denies in either mode, because every entity defaults to closed. Authentication is unchanged, and an applied plane with `mode: warn` still records decisions without refusing.

If the apply is skipped after an upgrade, a `warn` node runs without governance and a `deny` node refuses governed traffic. `cloud doctor` reports the mode and whether each plane is applied, and `admin config planes list` shows an `applied` column.

## 3. Rebuild services bundles

A bundle packed for 0.72 carries the old catalog shape, `roles.yaml`, marketplace `access:` ingestion and possibly `hooks.evaluation`. The 0.73 loader refuses it. Run `config migrate` on the source tree, then repack and republish every bundle with the 0.73 CLI.

At boot the bundle bootstrap validates the composed tree before it replaces the current one. A bundle the binary cannot load is refused with the bundle names and the loader's file, line and rule. `on_fetch_failure` then serves the last good tree, then the bundled one. `/readyz` (`config.services_source`), `/health` (`services_source`, status `degraded`) and `cloud doctor` report the degraded source until a composition loads. `services refresh` retries a composition whose reconcile did not complete.

## 4. Database migrations

`systemprompt infra db migrate` applies these migrations. No existing migration file changed. Migrations are forward-only, `.down.sql` files are not run during an upgrade, and `.tombstone` files reserve slot numbers without SQL.

A large table is one that holds tens of millions of rows on an established install.

| Crate | Migration | Change | Lock and cost |
|---|---|---|---|
| agent-runs | `001_agent_runs_host_cli_ids` | `opencode` rows become `opencode-cli`; CHECK on CLI host ids | One pass over `agent_runs`; CHECK `NOT VALID` then `VALIDATE` |
| agent-runs | `002_agent_runs_harness` | `harness_digest`; `harness_unavailable` error class | Catalog-only |
| agent-runs | `003_agent_runs_idempotency` | `idempotency_key`, `request_digest`, partial unique index | Index built in-transaction on `agent_runs` |
| scheduler | `005_config_plane_last_apply` | Last-apply columns on `config_plane_state` | Catalog-only |
| scheduler | `006_config_change_audit_merge_mode` | Audit mode check admits `merge` | `NOT VALID` then `VALIDATE` |
| ai | `046_ai_requests_status_check` | CHECK on `ai_requests.status` | Large table; `NOT VALID` then `VALIDATE` |
| ai | `047_ai_quota_buckets_subject_kind_check` | CHECK on `subject_kind` | Large table; `NOT VALID` then `VALIDATE` |
| ai | `048_retention_indexes` | Retention indexes on message and payload tables | Large tables; built `CONCURRENTLY`, writes continue. Build time grows with table size |
| ai | `050_ai_gateway_policies_change_provenance` | Provenance columns on `ai_gateway_policies` | Small table |
| ai | `051_catalog_credentials` | `user_provider_credentials.provider` becomes `credential`; nullable `ai_requests.credential` and `endpoint`; `ai_catalog_legacy_map` | Catalog-only `ADD COLUMN` on `ai_requests`. The backfill runs from `config migrate --data` in bounded batches |
| ai | `052_ai_requests_traffic` | `traffic` (`production` or `evaluation`) replaces `synthetic` on `ai_requests` and the hourly rollup | Defaulted `ADD COLUMN`; CHECK `NOT VALID` then `VALIDATE`; the re-class touches only evaluation rows |
| ai | `053_tool_call_args_validity` | `args_validity`, `args_error_path` on tool calls | Catalog-only; CHECK `NOT VALID` then `VALIDATE` |
| ai | `054_ai_context_rollup_hourly` | Per-conversation hourly rollup | Queues existing hours for the `backfill_rollups` job |
| marketplace | `018_publication_gate` | Gate columns on publication reviews | Small table |
| marketplace | `019_publication_cohorts` | Cohort selections | Small table |
| marketplace | `020_managed_rule_kind` | `rule` resource kind | Small table |
| oauth | `021_bridge_host_prefs_host_check` | Deletes host-preference rows naming an unknown host; CHECK on host ids | Small tables |
| users | `026_session_source_default_unknown` | `session_source` defaults to `unknown` | Catalog-only |
| users | `027_session_traffic_rollup_hourly` | Session traffic rollup and triggers | Brief lock on `user_sessions` |
| users | `028_session_traffic_rollup_backfill` | Counts existing sessions into the rollup | One pass over `user_sessions`; writes continue |
| users | `029_api_key_sessions` | `api_key_sessions` | Small table |
| database | `006_services_status_check` | CHECK on `services.status` | Small table |
| governance | `026_access_control_change_provenance` | Provenance columns on access-control tables | Catalog-only |
| governance | `027_entitlement_kinds_closed_source` | `model` and `provider` entity kinds; closed `source` vocabulary | Small tables; `NOT VALID` then `VALIDATE` |

The `users/028` and `ai/048` migrations take time in proportion to `user_sessions` and the message tables. Their statement timeout is projected from a 2M-row baseline and capped at 30 minutes. On an installation with tens of millions of rows, run the upgrade attended with `SYSTEMPROMPT_MIGRATION_STATEMENT_TIMEOUT_SECS=0`.

A new CHECK fails its migration if existing rows hold values outside its vocabulary. Run these queries on a restored copy first:

```sql
SELECT status, count(*) FROM ai_requests GROUP BY 1;   -- pending, completed, failed, rejected
SELECT status, count(*) FROM services GROUP BY 1;      -- starting, running, stopping, stopped, error
SELECT host, error_class, count(*) FROM agent_runs GROUP BY 1, 2;
```

Queries that read `ai_requests.synthetic` must filter `traffic = 'production'` instead of `NOT synthetic`. A writer that inserts `user_sessions` rows meaning `web` must bind `session_source` explicitly.

## 5. Upgrade order

1. **Back up the database** and confirm the backup restores. Rehearse steps 3 to 6 on a restored copy.
2. **Bump the pins.** Set every `systemprompt*` crate to `0.73.0` together and update `Cargo.lock`. See [section 7](#7-api-changes-for-crate-consumers).
3. **Convert the profiles, then the tree.** Run `config migrate --profiles <dir>` (see [1.1](#11-profiles)) and commit the result. Then run `config migrate --check` and `config migrate`, review the diff, and commit it with `services/ai/legacy_map.yaml`. Fix host ids. Repack and publish any services bundles.
4. **Migrate the database:**
   ```bash
   systemprompt infra db migrate --profile <name>
   ```
   Pass `--profile` explicitly. The command refuses a cloud profile that was selected implicitly.
5. **Move stored data:**
   ```bash
   systemprompt admin config migrate --data --profile <name>
   ```
   This fills `ai_catalog_legacy_map`, rewrites personal-key credential ids, moves access rules onto their declared route ids and model entities, and backfills `ai_requests.credential` and `endpoint` in bounded batches. It needs migration `ai/051`.
6. **Apply configuration:**
   ```bash
   systemprompt admin config apply --reason "upgrade to 0.73.0" --profile <name>
   ```
   Resolve any reported conflict and run the apply again until it exits zero.
7. **Deploy** the 0.73.0 server, then roll out the 0.73 bridge. A 0.72 bridge cannot use a 0.73 gateway, and a 0.73 bridge cannot use a 0.72 gateway.
8. **Verify:**
   - `/readyz` is ready, and `/health` reports no `governance` or `services_source` degradation;
   - `cloud doctor` shows `governance`, `gateway_policies` and `access_control` as applied;
   - `config migrate --check` reports nothing pending;
   - one request per client family is served, and its audit row records `credential`, `endpoint` and `traffic`;
   - `GET /v1/bridge/profile` with a user bearer lists the expected models;
   - personal keys still verify.

## 6. Behaviour an operator will notice

- `GET /v1/bridge/profile` returns 401 for a missing, invalid or stale bearer. There is no anonymous profile.
- A personal access token carries its user's roles, groups and projects, so calls that returned 403 with a token on role-gated routes now succeed.
- The session id is bound to the credential. A JWT request whose `x-session-id` names a different gateway session than its `session_id` claim returns 401. A client's own session id is recorded as `client_session_id`.
- Personal keys are stored per catalog credential: `/v1/me/provider-credentials/{credential}`.
- `GET /api/v1/agent/tasks?status=<unknown>` returns 400. `cancelled` is not accepted. Use `canceled`.
- `admin config gateway default-provider` is `default-model`. `admin config credential|provider|endpoint|model|deployment` replace the providers editor.
- The OpenCode integration writes the V2 configuration only and needs `@opencode/cli` 2.0.30 or later.

## 7. API changes for crate consumers

This section summarises the changes. The changelogs give each signature and its migration.

- **Owned state instead of process globals.** Signing keys (`AppContext::signing_keys()`), event broadcasters (`event_broadcasters()`), the subprocess spawner (`spawner()`), files config (`files_config()`), logging (`logging()`), the connector token cache (`connector_tokens()`) and the gateway engines (`GatewayEngines`) are built once by the application context and passed explicitly. The `::global()` accessors, `init`/`get` cells and free functions they replace are removed. Readiness and scheduler health are values owned by the server.
- **Boot context instead of bootstrap statics.** `ProfileBootstrap`, `SecretsBootstrap`, `ServicesRootBootstrap` and `Config::get` are removed. The profile, the live secrets, the resolved `Config` and the active services root are one owned `BootContext` (`systemprompt_loader`): a binary builds it once, `AppContextBuilder` takes it, and code reads it from `AppContext::boot()` or the value it was handed. `AppContext::token_authority()` (`TokenAuthority`, in `systemprompt_models::auth`) verifies and signs tokens. Use it rather than holding `SigningKeys`.
- **Configuration planes.** `AppContext::governance()` returns the current engine, rebuilt when a plane changes. `CredentialsBootstrap::load()` replaces `init`/`get`.
- **Catalog.** `ProviderEntry`, `ProviderModel`, `AiProviderConfig`, `ApiSurface` (now `ClientFamily`) and `ServicesBootstrap::providers()` are removed. Use `ResolvedCatalog`, read from the services state.
- **Services state.** The process-global services config is removed: `ServicesBootstrap` and the `ConfigLoader` result cache are gone, and `ConfigLoader::load` reads the tree on every call. `systemprompt_loader::ServicesState` owns the services config watch, the `ResolvedCatalog` and the boot discovery report. Build it once with `ServicesState::load` (or `load_from_path`, `load_with_discovery`, `from_config`), hand a prebuilt one to the builder with `AppContextBuilder::with_services`, and read it through `AppContext::services_state()`. Consumers do not hold the state itself: `ServicesState::view` builds a `ServicesView` (the services config watch and the `ResolvedCatalog`), and registries, deployment services, the agent orchestration, the proxy and the hook and factory contexts (`AuthzHookContext`, `SsoHookContext`, `MarketplaceFactoryContext`) take that view.
- **Token cache and CLI output.** The provider token cache is a value: `ProviderCredential::bearer`, the upstream resolvers and `GatewayEngines::new` take it, so pass `ctx.connector_tokens()`; extensions reach it as `ProviderTokenCache` through `ExtensionContext::provider_tokens()`. The CLI output mode, structured-output flags and notice buffer are methods on an `OutputState` installed with the tracing subscriber.
- **Extensions.** `ExtensionContext` gains the required `background_tasks()`, `services() -> ServicesView` and `provider_tokens() -> Arc<dyn ProviderTokenCache>`; `ProviderTokenCache` is a new trait in `systemprompt_traits`, implemented by `TokenCache`. `ExtensionContext` also gains `subject_attributes() -> Arc<dyn SubjectAttributeSource>`: `SubjectAttributeSource`, `SubjectAttributes`, `SubjectDimension` and `RuleType` live in `systemprompt_models::authz`, and governance re-exports them. A gateway guard factory takes the context (`fn(&dyn ExtensionContext) -> Arc<dyn GatewayRequestGuard>`, `register_gateway_guard!(factory = f)`), and the process-wide `gateway_guards()` and `run_gateway_guards()` are replaced by `GatewayGuards`, owned by `GatewayEngines`. Extensions register only through `register_extension!`, so `RuntimeBuilder::with_extension` and the injected-extension functions are removed.
- **Extension context.** An extension reads the deployment through `ExtensionContext`: `profile()`, `app_config()`, `services_root()` (the `ActiveServicesRoot` with its provenance), `token_authority()`, `secrets()` (a live `SecretStore`, read at request time so reloads reach it; it replaces `secret(name)`) and `dependencies()` (the pool, paths, files config, OAuth and AI repositories). The content-pipeline hooks (`page_prerenderers`, `page_data_providers`, `component_renderers`, `template_data_extenders`, `content_data_providers`) take the context, so build site configuration inside the hook, not at construction. Safety scanner factories take `(ctx, settings)`. `SsoHookContext`, `AuthzHookContext` and `MarketplaceFactoryContext` carry the secrets store, and `HookTokenValidator::new` takes the `TokenAuthority`. The registration macros resolve through `$crate`, so a crate that reaches core only through the `systemprompt` facade needs no direct `inventory` or `systemprompt-extension` dependency.
- **Models and identifiers.** `HostKind` has seven ids and is shared by the bridge and agent runs. `ClientSessionId` is an opaque token. `EnforcementMode` replaces `PolicyMode`, `SafetyMode` and `QuotaMode`. `JwtClaims`, `CanonicalRequest`, `AiRequest` and the session-store parameters group their fields into named sub-structs, with unchanged wire shapes.
- **Errors.** `ApiError::internal(context, source)` is the only 500 constructor. `AiError` nests provider, routing, request, tool and storage errors.
- **New struct fields.** `RunRequest.harness` and `traffic`, `PublicationRequest.cohort`, `ServicesConfig.marketplace`, `SettledToolCall.args`, `RuleEntry` and `PluginEntry.publication`, and `ManifestMarketplace.opencode` need values in struct literals.

## Rollback

Migrations are forward-only, and `ai/052` drops the `synthetic` columns that 0.72 reads, so a 0.72 binary cannot run against a migrated database. To roll back, restore the backup from step 1 and redeploy 0.72 with the previous services tree. Data written after the upgrade is lost, so decide whether to proceed before reopening traffic.
