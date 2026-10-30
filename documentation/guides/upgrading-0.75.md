# Upgrading to 0.75

How to move a production deployment from 0.74 to 0.75.0. This release splits the server role so the API tier can scale on its own, makes document actors follow the credential, adds resumable uploads and an identity config plane, and adds self-managed Kubernetes hosting. An operator has five things to do: run nineteen forward-only migrations, rename any `server.role: admin`, check the secrets source and config-plane authority in each profile, deploy the server and the bridge together if the bridge Drive is in use, and update clients that read document error bodies. The full list of changes, with a migration note for each, is under `### Breaking` in `CHANGELOG.md` and the per-crate changelogs.

## At a glance

| Area | Action | Required |
|---|---|---|
| Database | 19 forward-only migrations | Yes. Run before serving |
| `server.role: admin` | Replace with `api` and `worker`, or `all` | Yes, if a profile names `admin` |
| `database.pool.max_connections_by_role` | Rename the `admin` key to `api` and `worker` | Yes, if set |
| `secrets.source: file` | Now read on every host. Set `source: env` where secrets come from the environment | Review |
| `config_planes.authority` | An unnamed plane now runs under `db`, not `file` | Review |
| `cloud.provider` | New, optional. Defaults to `systemprompt` | No |
| `licence` | New, optional | No |
| Identity plane | New, optional `services/identity/*.yaml` | No |
| `config apply` | Run as after every release | Yes, as in 0.74 |
| Bridge Drive | Deploy the server and roll out the 0.75 bridge together | Yes, if the Drive is used |
| Document API clients | Read `error_key` and `detail`; `restore` is `untrash` | Yes, if you call the document API |
| Collaborative editor | Core no longer ships the Loro client or the editor component | Yes, if you serve the editor |
| Rust consumers | Many new fields and variants, moved document modules, `NodeRole` split | Only if you build on the crates |

## 1. Database migrations

`systemprompt infra db migrate` applies these migrations. No existing migration file changed. `014_document_grant_events_reserved` is a tombstone that holds its number and runs nothing.

Two of the tables on the large-table list are touched: `011` adds a nullable column to `document_revisions` and backfills it for the revision chain of every document head. The document store arrived in 0.74 and is off by default, so on most deployments that table is small or empty. Where the store has been in use, run the migration in a maintenance window sized to the number of revisions. No other migration rewrites a large table.

| Crate | Migration | Change | Lock and cost |
|---|---|---|---|
| database | `007_instance_endpoints` | `instance_endpoints`: the address each replica advertises with its heartbeat | New table |
| database | `008_instance_presence` | `instance_presence`: each replica's `server.role`, per heartbeat | New table |
| scheduler | `007_licence_checkin_state` | `licence_checkin_state` | New table |
| oauth | `022_oauth_client_declared_source` | `oauth_clients.declared_source`, `declared_secret_key` | Catalog-only `ADD COLUMN` |
| users | `030_users_declared_source` | `users.declared_source` | Catalog-only `ADD COLUMN` |
| users | `031_api_key_kind` | `user_api_keys.kind`, default `user` | Constant default, catalog-only |
| files | `011_document_revision_landing` | `document_node_counters`; `document_revisions.landed_at`, backfilled along each head's chain; `document_write_acks.user_id`, `request_sha256` | **Large table** `document_revisions`: catalog-only `ADD COLUMN`, then an `UPDATE` of every head-reachable revision |
| files | `012_document_collab_session_seq` | `document_edit_sessions.next_seq`, `snapshot_vv` | Small table; backfill of open sessions |
| files | `013_document_path_history` | `document_path_changes` | New table |
| files | `014_document_grant_events_reserved` | Tombstone | None |
| files | `015_document_grant_events_and_access_requests` | `document_grant_events`, `document_access_requests` | New tables |
| files | `016_document_comment_ranges` | Range anchors, `mentions`, `edited_at`, `deleted_at` on `document_comments` | Catalog-only `ADD COLUMN` |
| files | `017_document_uploads` | `document_uploads`, `document_upload_parts` | New tables |
| files | `018_document_file_format` | `documents.format` admits `file` | Small table; CHECK replaced, then `VALIDATE` |
| files | `019_document_upload_claims` | Upload `completing` status, `checked_kind`, `claimed_at` | Table new in this release |
| files | `020_document_upload_accounting` | `document_uploads.received_bytes` | Table new in this release |
| files | `021_document_upload_one_shot` | `document_uploads.one_shot` | Table new in this release |
| files | `022_document_upload_quota_exempt` | `document_uploads.quota_exempt` | Table new in this release |
| files | `023_document_mention_grants` | `reason`, `via_agent` on `document_grant_events`; `via_agent` on `document_comments` | Catalog-only `ADD COLUMN` |
| files | `024_document_agent_caps` | `agent_cap` on `documents` and `document_folders`, `documents.created_by_agent`, `document_agent_reads` | Catalog-only `ADD COLUMN`; new table |

The migrations run whether or not the document store is enabled. After the upgrade, the `document_node_ids_backfill` job mints node ids for any revision written before ids existed; it runs once per release. `cloud doctor` now fails when the database holds migrations newer than the binary, because migrations are forward-only.

## 2. Configuration

### 2.1 Server roles

`server.role: admin` is replaced by two roles. A profile that names `admin` is refused at boot with an error naming both.

- `api` serves the console, agent, MCP, content, stream, messaging, extension and static routes. It runs no scheduler and no MCP or agent processes, so it scales horizontally.
- `worker` runs the scheduler and owns the MCP and agent processes. It requires `server.advertise_url`, the address other replicas use to reach it. Each MCP server and agent is leased to one worker, and an `api` replica forwards to the owner. A request that finds no live owner answers `503 no_live_owner`.

Split an `admin` deployment into an `api` deployment and a `worker` deployment, or keep `all` for a single node. `database.pool.max_connections_by_role` takes `api` and `worker` keys instead of `admin`. An `api` or `worker` profile with `storage.backend: local` must set `storage.shared: true`.

### 2.2 Secrets source

A profile with `secrets.source: file` reads its secrets file on every host. In 0.74 a deployment host that had `OAUTH_AT_REST_PEPPER` in its environment used environment secrets instead. A profile that relies on environment secrets sets `source: env`.

### 2.3 Config-plane authority

A plane the profile does not name under `config_planes.authority` now runs under `db` authority; in 0.74 it ran under `file`. A plane whose declaration file is absent is `undeclared` whatever the profile says. To keep a plane managed in Git, name it `file` in the profile:

```yaml
config_planes:
  authority:
    identity: file
```

The new `identity` plane declares users, service accounts and static OAuth clients in `services/identity/*.yaml`. It reads and writes only rows a declaration created or adopted, so existing users and runtime-registered clients are left alone. While it is under `file` authority, `admin users` write commands and the `/oauth/clients` write endpoints answer `409 config_plane_file_managed`.

### 2.4 Hosting provider and licence

`cloud.provider` names where a profile is hosted: `systemprompt` (the default), `kubernetes`, `gke` or `docker_local`. It selects the deployment-host markers, the trusted-proxy preset and the `cloud doctor` proxy check. A `kubernetes` or `gke` profile must set its trusted proxies explicitly. Existing `tenants.json` records load unchanged and are rewritten with `provider` on the next save.

`licence` is optional. With `licence.multi_tenant: true` a profile runs a report-only check-in job; `admin licence status` shows its last result.

### 2.5 The document store

The settings in 0.74 still apply. New optional keys under `files.documents` configure uploads (`uploads`), access requests (`access_requests`) and limits, including the per-agent and per-user agent edit ceilings and `pdf_pages`. Every document surface now derives its actor from the credential: only a first-party browser session cookie acts as the person, and bridge, PAT, API, MCP and download-link credentials act as agents, subject to the owner's agent caps.

## 3. Upgrade order

1. **Back up the database** and confirm the backup restores.
2. **Bump the pins.** Set every `systemprompt*` crate to `0.75.0` together and update `Cargo.lock`. See [section 5](#5-api-changes-for-crate-consumers).
3. **Update profiles.** Replace `server.role: admin` (see [2.1](#21-server-roles)), rename `max_connections_by_role.admin`, and check `secrets.source` and `config_planes.authority` (see [2.2](#22-secrets-source) and [2.3](#23-config-plane-authority)).
4. **Migrate the database:**
   ```bash
   systemprompt infra db migrate --profile <name>
   ```
   Pass `--profile` explicitly. The command refuses a cloud profile that was selected implicitly.
5. **Apply configuration:**
   ```bash
   systemprompt admin config apply --reason "upgrade to 0.75.0" --profile <name>
   ```
   `admin config planes apply --dry-run` shows what the apply would change and writes nothing.
6. **Deploy** the 0.75.0 server. Roll out the 0.75 bridge straight after if the Drive is in use.
7. **Verify:**
   - `/readyz` is ready and `/health` reports no degradation;
   - `cloud doctor` shows every plane as applied and reports no migrations ahead of the binary;
   - one request per client family is served, including an MCP tool call through an `api` replica;
   - if the document store is enabled, `GET /api/v1/documents` answers for a signed-in user and `bridge drive status` reports the Drive.

### 3.1 Bridge and gateway compatibility

The minimum bridge version the server requires is unchanged, and a 0.74 bridge keeps syncing, enrolling and proxying against a 0.75 server. The Drive is the exception, in both directions:

- A 0.75 server refuses a plain bridge token on `/v1/bridge/drive/*`. A client first trades a device token at `POST /v1/bridge/drive/token`. A 0.74 bridge does not do this, so its Drive sync stops until the bridge is upgraded.
- A 0.75 bridge needs the `device_token` that a 0.75 server returns from the bridge exchange. Against a 0.74 server it has none, and its Drive sync does not run.
- A Drive commit from an agent capped at `suggest` answers `201` with `status: proposed`. The 0.75 bridge keeps the local file and shows it as sent for review.

If the Drive is in use, deploy the server and roll out the 0.75 bridge in the same window.

## 4. Behaviour an operator will notice

- Document API errors use the refusal code as `error_key` (`stale_base`, `guard_mismatch`, `locked`, `not_found`, …) and carry typed detail under `detail`. `revision_conflict`, `stale_anchor`, `lock_held`, `edit_rejected`, `malformed_document` and `unknown_node` are gone. A trashed document refuses `trashed`, and `template_failed` answers `422`.
- `POST /api/v1/documents/{id}/restore` is `POST /api/v1/documents/{id}/untrash`, and the `documents_restore_document` MCP tool is `documents_untrash`.
- `documents_history` items carry `author`, `actor` and `at`.
- `own` is no longer a grantable level. A share of `own` is refused, and ownership moves by transfer. A share to an unknown subject is refused with `unknown_subject`.
- Core no longer ships the vendored `loro-crdt` WebAssembly, the `documents-collab.js` client or the `sp-document-editor` component. An installation that serves the collaborative editor vendors a Loro client at `systemprompt_files::documents::engine::loro::WIRE_VERSION`.
- A web login that sets the session cookie mints a token with the `browser_session` claim. A token without the claim sent as the cookie is refused `401`.
- An MCP output marked `terse_wire` carries only `artifact_id` and `mcp_execution_id` under `io.systemprompt/execution` in `_meta`.
- An enabled SSO provider whose configuration is unusable, or two providers with one id, fail boot instead of leaving the provider off.
- A token verifies only when its header `alg` equals the key's algorithm. The published `/.well-known/jwks.json` is unchanged.

## 5. API changes for crate consumers

This section summarises the breaking changes. The changelogs give each signature.

- **Server roles.** `NodeRole::Admin`, `serves_admin` and `runs_workers` are removed. Use `NodeRole::{Api, Worker}`, `serves_api`, `runs_scheduler`, `supervises_processes` and `writes_shared_caches`. `ServerConfig` and `Config` gain `advertise_url`, and `RoleMaxConnections::entries` returns five entries.
- **Manifest.** `Profile` gains `licence`, `CloudConfig` gains `provider`, `deploy` and `operator`, and `ProfileError` gains `ReadOnly`.
  ```rust
  Profile { /* .. */, licence: LicenceConfig::default() }
  CloudConfig { /* .. */, provider: HostingProvider::default(), deploy: None, operator: None }
  ServerConfig { /* .. */, advertise_url: None }
  ```
- **Cloud and CLI.** `TenantType` is replaced by `HostingProvider`, `StoredTenant::tenant_type` by `provider`, and `is_cloud` by `is_systemprompt_managed` and `is_self_managed`. `default_cloud_trusted_proxies` and `default_local_trusted_proxies` become `default_trusted_proxies(HostingProvider)`. `DeployOrchestrator::deploy` takes a `DeployProvider`, `DeployRequest::credentials` becomes `platform_credentials`, `ExecutionTarget` gains `SelfManaged`, and `CloudError` gains provider, GitOps and operator variants.
- **Auth.** `CustomClaims`, `UserJwtRequest`, `JwtConfig` and `JwtUserContext` gain `browser_session` and `interaction`. `AuthContext` gains `cookie_transport` and `BuildContextParams` takes it. `BridgeAccessRequest` gains `interactive`. API key params and `UserApiKey` gain `kind: ApiKeyKind`.
  ```rust
  CustomClaims { /* .. */, browser_session: false, interaction: None }
  CreateApiKeyParams { /* .. */, kind: ApiKeyKind::User }
  ```
- **Security.** `Jwk` and `Jwks` are publish-only. Read an issuer's keys as `VerificationKeySet`; `JwksClient::fetch` returns `VerificationKey`. Verify with `key.decoding_key()` and `Validation::new(key.algorithm())`.
- **OAuth issuance.** `SsoProviderRegistration::factory` returns `Result<Option<_>, SsoProviderError>`: `Ok(None)` when the provider is not configured, `Err` when it is enabled but invalid.
- **Documents (files).** The Loro CRDT moves to the new `systemprompt-engine-loro` crate behind `DocumentEngine`. `CollabService::new` takes an `Option<Arc<dyn DocumentEngine>>`; pass `Some(Arc::new(LoroEngine))` and enable the facade's `files-collab-loro` feature (`full` implies it). `DocumentAccessService::new` takes a `SubjectDirectory`. Commits carry `op_id` and return `Committed`/`Merged(Box<LandedCommit>)`. Heads, node ids and write acknowledgements are written in one transaction from `DocumentRepository::begin`. `DocumentsConfig` gains `limits`, `uploads` and `access_requests`, and `DocumentsError`, `CollabError` and `DocumentFormat` gain variants.
  ```rust
  DocumentsConfig { /* .. */, limits: DocumentLimits::default(), access_requests: false, /* uploads */ }
  CommitRequest { /* .. */, op_id: None, media: None }
  ```
- **Runtime.** `RefusalCode` and `Refusal` move to `documents::refusal`, the read views to `documents::views`, and the download-link helpers to `documents::download`. `DocumentsRuntimeParts` gains `directory` and `background_tasks`, and `DownloadTarget` gains `rendition`.
- **Provider contracts.** `EditOp` gains rich-text and table operations, `CodecError` gains `Refused` and `Payload`, and Markdown `materialise` returns `Result`.
- **Other enums and structs.** `LifecyclePlan` gains `process_ownership`, `ProxyError` gains `NoLiveOwner`, `ExtensionMigrationStatus` gains `ahead`, `LoaderError` gains `InitializationStepFailed`, `BundleError` gains `NoSharedComposition`, `UserError` gains `AccountKindMismatch`, `SecretsBootstrapError` gains `LicenceSigningSecretRequired`, `ReconcileParams` gains `owned`, `merge_plane` takes `file_wins`, `DriveState::new` takes the `TokenAuthority`, and `DocumentSharedEvent` and `DocumentEventName` gain fields and variants. Add arms, or a wildcard, to exhaustive matches.
- **Deployment-host markers.** `subprocess::deployment_host` and `is_deployment_host` take the marker set to consult. `Profile::is_deployment_host` uses the set the profile's provider names.

## Rollback

Migrations are forward-only, and `.down.sql` files are not run. To roll back, restore the backup from step 1 and redeploy 0.74 with the previous profiles and services tree. Data written after the upgrade, including documents and uploads, is lost, so decide whether to proceed before reopening traffic.
