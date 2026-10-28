# Upgrading to 0.74

How to move a production deployment from 0.73 to 0.74.0. This release adds a shared document store, which is off by default. An operator has three things to do: run ten forward-only migrations, decide whether to enable the document store, and deploy the server before the bridge. 0.74 has no profile migration and no services-tree conversion. The full list of changes, with a migration note for each, is under `### Breaking` in `CHANGELOG.md` and the per-crate changelogs.

## At a glance

| Area | Action | Required |
|---|---|---|
| Database | 10 forward-only migrations | Yes. Run before serving |
| Profiles and services tree | None. A 0.73 tree loads unchanged | No |
| `config apply` | Run as after every release | Yes, as in 0.73 |
| Document store | `files.documents.enabled: true` in `services/config/files.yaml` | No. Off by default |
| `prompt_caching` | Now defaults to on. Set `prompt_caching: false` to opt out | Review |
| `external_agents.<host>.default_enabled` | New, optional | No |
| Marketplaces with `opencode:` themes | Each declared theme must ship in a plugin at its pinned digest | Yes, if you declare themes |
| Bridge clients | Deploy the 0.74 server first. A 0.73 bridge keeps working against it | Order matters |
| Rust consumers | New enum variants, typed `ContentHash` and `ModelId`, and a `documents` field on `Subsystems` | Only if you build on the crates |

## 1. Database migrations

`systemprompt infra db migrate` applies these migrations. No existing migration file changed. Every migration touches only tables that 0.74 introduces or small access-control tables, so none of them rewrites a large table.

| Crate | Migration | Change | Lock and cost |
|---|---|---|---|
| files | `003_documents_tables` | Document store tables: folders, blobs, documents, revisions, grants, locks, presence, edit sessions, text index | New tables |
| files | `004_document_node_ids_and_attribution` | `document_node_ids`, `document_write_acks`; attribution columns on `document_revisions` | New tables; catalog-only `ADD COLUMN` |
| files | `005_document_collab_updates` | `document_collab_updates` | New table |
| files | `006_document_revisions_context_index` | Index on `document_revisions (context_id, created_at)` | Built `CONCURRENTLY` in a no-transaction file; writes continue |
| files | `007_document_comments_and_trash` | `document_comments`; trash columns on `documents` | New table; catalog-only change on `documents` |
| files | `008_document_proposals` | `document_proposals`; `document_folders.agent_edit_policy` | New table; catalog-only change |
| files | `009_document_grants_subject_display` | `document_grants.subject_display` | Catalog-only |
| files | `010_documents_deleted_at` | Trash timestamp stored in `documents.deleted_at` | Catalog-only rename; index on new table |
| governance | `028_document_entity_kinds` | `document` and `folder` entity kinds on the access-control CHECKs | Small tables; `NOT VALID` then `VALIDATE` |
| governance | `029_entity_kinds_complete` | The access-control CHECKs admit every entity kind the code defines | Small tables; `NOT VALID` then `VALIDATE` |

The migrations run whether or not the document store is enabled.

## 2. Configuration

### 2.1 The document store

The document store is disabled by default. To enable it, set the switch in `services/config/files.yaml`:

```yaml
files:
  documents:
    enabled: true
    maxUploadBytes: 67108864     # default 64 MiB; between 1 MiB and 2 GiB
    blobGcGraceHours: 24         # default
    presenceTtlSecs: 90          # default
    trashRetentionDays: 30       # default; 0 is refused
```

While the store is enabled, the server mounts `/api/v1/documents`, `/api/v1/document-folders` and the bridge Drive routes under `/v1/bridge/drive/`. It also registers missing folder authz entities at boot and runs the blob garbage collector, the presence sweep, the trash purge and the collab update sweep jobs. While it is disabled, those routes are not mounted and the bridge reports the Drive as disabled. The documents MCP tools refuse to build and name the switch.

Document access is decided by grants and by access-control rules on the new `document/<id>` and `folder/<id>` entities, through the authz hook. A failed decision denies access.

### 2.2 Prompt caching defaults to on

A provider or model that does not state `prompt_caching` now gets automatic prompt caching on Anthropic-wire endpoints. To keep the 0.73 behaviour for a provider or model, set it explicitly:

```yaml
prompt_caching: false
```

### 2.3 Opt-in hosts

`external_agents.<host>.default_enabled: false` makes an enabled host opt-in. Such a host reaches only users with an explicit enabled row, and it does not count towards the rule that a user with no rows gets every host. A tree that omits the key behaves as in 0.73.

### 2.4 Marketplaces with OpenCode themes

A marketplace's `opencode:` block can declare several `themes` with `default_theme` and `theme_mode`. The single `theme:` form is still read as the only theme and the default, so an existing tree loads unchanged. `theme` is no longer allowed in `cli_defaults`.

`core marketplace import` now refuses a marketplace that declares a theme no plugin ships under `hosts/opencode/themes/<name>.json` at the pinned sha256. With `--strict`, it also refuses a marketplace that configures `opencode:` while a rule's `hosts` reach no OpenCode host, and it refuses a skill directory without `SKILL.md`, a cited `references/…` path the skill does not carry, or a `../` path out of the skill directory. Run the import against each marketplace before you deploy.

## 3. Upgrade order

1. **Back up the database** and confirm the backup restores.
2. **Bump the pins.** Set every `systemprompt*` crate to `0.74.0` together and update `Cargo.lock`. See [section 5](#5-api-changes-for-crate-consumers).
3. **Check marketplaces.** Run `core marketplace import` on each marketplace that declares `opencode:` themes (see [2.4](#24-marketplaces-with-opencode-themes)).
4. **Migrate the database:**
   ```bash
   systemprompt infra db migrate --profile <name>
   ```
   Pass `--profile` explicitly. The command refuses a cloud profile that was selected implicitly.
5. **Apply configuration:**
   ```bash
   systemprompt admin config apply --reason "upgrade to 0.74.0" --profile <name>
   ```
   `admin config planes apply --dry-run` shows what the apply would change and writes nothing.
6. **Deploy** the 0.74.0 server, then roll out the 0.74 bridge.
7. **Verify:**
   - `/readyz` is ready and `/health` reports no degradation;
   - `cloud doctor` shows every plane as applied;
   - one request per client family is served;
   - if the document store is enabled, `GET /api/v1/documents` answers for a signed-in user and `bridge drive status` reports the Drive.

### 3.1 Bridge and gateway compatibility

Deploy the server first. A 0.74 server works with a 0.73 bridge:

- The bridge manifest's `opencode:` block gains `themes`, `default_theme` and `theme_mode`. A 0.73 bridge ignores keys it does not know, so it syncs as before but does not apply the marketplace's OpenCode theme.
- The minimum bridge version the server requires is unchanged.
- A 0.74 gateway is more lenient than 0.73 about `x-session-id`. A bridge or OAuth JWT whose header differs from its `session_id` claim lands on the claim's session, and the header is recorded as `client_session_id`.

A 0.74 bridge against a 0.73 server still syncs and reads the single `theme:` form. It reports the Drive as disabled, because a 0.73 server has no Drive routes. It is also subject to the 0.73 rule that refuses a JWT whose `x-session-id` names another gateway session.

`GET /v1/bridge/latest` now offers only bridge releases whose major.minor is no higher than the server's own version. Once the server runs 0.74, its clients are offered the 0.74 bridge. If no supported release is listed, the route answers 404 `compatible_release_not_found`. A `pinned_version` still overrides.

## 4. Behaviour an operator will notice

- The bridge sets up only hosts that the delivered manifest enables. On first use it syncs before it touches any host, and with no manifest it sets up no host. `--host claude-code` respects the instance's enabled hosts, and a host suppressed by policy is never enrolled or written.
- A sync removes a Claude Code `managed-mcp.json`, or strips its MCP allow-list keys, only when every server it names is the bridge's own loopback MCP endpoint. A policy deployed by anyone else is left in place, and `bridge doctor` reports it.
- Claude Code's settings, plugins and install probe follow `CLAUDE_CONFIG_DIR` when it is set.
- An agent run's personal access token is bound to the run's gateway session, so every gateway request the run makes lands on that session. For an OpenCode run, the authoritative tokens and cost are the gateway's settled counters on that session.
- An MCP tool result for a client that negotiates structured content but not the MCP Apps UI extension carries the output as JSON in its text block after the summary.
- The authz webhook retries once, after 150 ms, when the connection itself fails. Any other failure is still denied at once.
- `core services validate` warns when the tree's OpenCode rules render to more than the 24,000 characters OpenCode reads from `AGENTS.md`.
- Document previews answer with same-origin frame options, so admin pages on the same origin can embed them in a sandboxed iframe.

## 5. API changes for crate consumers

This section summarises the breaking changes. The changelogs give each signature.

- **New enum variants.** `AssetType::Wasm`, `ErrorCode::PayloadTooLarge` (413) and `ErrorCode::Locked` (423), and `ImportWarning::RulesMissOpenCode`, `MissingSkillManifest`, `MissingReference` and `SiblingSkillPath`. Add arms, or a wildcard, to exhaustive matches. The new error codes are built with `ApiError::payload_too_large` and `ApiError::locked`.
- **Runtime.** `Subsystems` gains `documents: Arc<DocumentsRuntime>`. Code that builds `Subsystems` by hand for `AppContext::from_parts` passes one:
  ```rust
  // 0.73
  Subsystems { /* .. */ }
  // 0.74
  Subsystems { /* .. */, documents: Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts { /* .. */ })) }
  ```
  An extension's `Dependencies` carry `Arc<DocumentService>` while `files.documents.enabled` is on, and the entry is absent otherwise.
- **Host preferences.** `EnabledHostPrefs::any_enabled_row` is renamed `unknown_enabled_row`. `admits` and `BridgeHostPrefsRepository::set_enabled` take `&InstanceHosts` instead of a host slice:
  ```rust
  // 0.73
  repo.set_enabled(user_id, host, enabled, &hosts).await?;
  // 0.74
  repo.set_enabled(user_id, host, enabled, &services_config.instance_hosts()).await?;
  ```
- **OpenCode themes.** `ManifestOpenCode::theme` and `OpenCodeMarketplaceConfig::theme` become `themes: Vec<_>` with `default_theme` and `theme_mode`. Read the default with `default_theme_ref()`. The wire and YAML forms still accept a single `theme:`.
- **Bridge models.** `bridge::opencode::Lockdown` gains `allowed_plugins`. `bridge::opencode::ModelEntry` gains `name: Option<String>`. `BridgeProfileResponse` gains `model_names: BTreeMap<ModelId, String>`, and its `models` and `model_limits` are keyed by `ModelId` instead of `String`. The wire form is unchanged.
  ```rust
  // 0.74 struct literals
  Lockdown { /* .. */, allowed_plugins: &[] }
  ModelEntry { /* .. */, name: None }
  BridgeProfileResponse { /* .. */, model_names: BTreeMap::new() }
  ```
- **Typed content hashes.** A services bundle's content hash is a `ContentHash`. This covers `ServicesBundleManifest::content_hash`, `BundleSourceState`, `BundleMember`, `BundleProvenance`, the return of `compute_content_hash`, `BundleCache::bundle_dir(&ContentHash)` and `owning_bundle_hashes`. Build values with `ContentHash::try_new(s)?` and read them with `as_str()`. The serialised form is unchanged.
- **Gateway API.** `ModelAdmission::retain` takes and returns `Vec<ModelId>`. `routes::gateway::messages::session_binding::is_client_owned` is removed.

## Rollback

Migrations are forward-only, and `.down.sql` files are not run. To roll back, restore the backup from step 1 and redeploy 0.73 with the previous services tree. Data written after the upgrade, including documents, is lost, so decide whether to proceed before reopening traffic.
