# Changelog

## [0.73.0] - Unreleased

### Breaking

- `Profile::load_layered`, `Profile::load_from_path` and `Profile::from_layers` load `profiles/<env>/profile.yaml` over `profiles/base.yaml` (`merge_yaml`: maps merge, sequences replace). `Profile::from_yaml` layers the overlay text over the base beside it.
- `ServerConfig` gains `external_url` and `extra_cors_origins`; `api_server_url`, `api_internal_url`, `api_external_url`, `use_https`, `cors_allowed_origins`, `SecurityConfig::issuer` and `AuthzHookConfig::url` are `#[serde(skip)]` and filled by `Profile::derive_fields`. Stating one is `ProfileError::DerivedField`. `ServerConfig::new` builds one from host, port and external URL.
- `PathsConfig` gains `root`; every other relative path resolves against it.
- `Profile::save_overlay` / `create_overlay` and `profile::overlay::apply_changes` write edits to the overlay only.
- Removed `ProfileInfo`, `ProfileError::MovedToServices`, `MOVED_SECTIONS` and the `evaluation` alias. Legacy shapes are `ProfileError::LegacyShape` with `MIGRATE_COMMAND`. New `ProfileError::{MissingOverlay, WriteFile, InvalidExternalUrl}`.
- `Profile::validate_committed` / `committed_source_errors` refuse absolute temp paths in committed profile layers.

### Fixed

- `ProviderEntry::bills_same_credential`: two entries bill the same account when both use deployment secrets or both use the one personal key named by the same `credential_owner()`.

## [0.72.1] - 2026-10-22

### Breaking

- `ProviderEntry` gains `credential_from: Option<ProviderId>`, so struct literals need the new field (`None` keeps a provider on its own key).

### Added

- `ProviderEntry::credential_owner` names the entry whose personal key a provider uses, and `takes_own_personal_key` is true only for the entry that owns that key. The bridge profile reports a shared entry as configured when its owner's key is stored.

## [0.71.0] - 2026-10-21

### Added

- Providers accept `credential: platform | user`, `user_agent` and `forward_headers`; `ProviderEntry::is_user_credential`, `platform_secret` and `forwards_header`.
- `Secrets::custom_secret` reads a custom secret from the profile's secrets source.

### Changed

- **Breaking:** `ProviderEntry::api_key_secret` is `Option<SecretName>` and `ProviderEntry` gains `credential`, `user_agent` and `forward_headers`.
- **Breaking:** `profile::PlaneAuthority` is re-exported from `systemprompt-provider-contracts`.

## [0.70.0] - 2026-10-18

### Added

- Profile sections `config_planes.authority`, `server.shutdown`, `observability.log_format`, `secrets.refresh_secs`, `security.api_keys.max_lifetime_days`, `database.pool.max_connections_by_role`/`session_max_connections`, `paths.data`, `storage.backend: s3`, `governance.quota.shared_subject_shards` and `scheduler.unlisted_jobs`.
- `database_session_url` secret.

### Changed

- **Breaking:** `security.allow_registration` and `security.allow_dynamic_client_registration` default to `false`.
- **Breaking:** `storage.shared: true` requires `paths.data`; an S3 bucket name containing `_` or `.` requires `path_style: true`.
- **Breaking:** gateway `hidden_requires_route` defaults to `true`.

## [0.69.0] - 2026-10-15

### Changed

- `ServicesConfig::instance_enabled_hosts` owns the instance-enabled bridge host rule.

## [0.67.0] - 2026-10-13

### Added

- `profile.secrets.source: external` and `profile.secrets.external: { kind, refresh_secs, <provider config> }` (`ExternalSecretsConfig`); `SecretsConfig::external` and `SecretsSource::External`, with `SecretsSource::source_name` for messages.

### Changed

- `BUNDLE_ALLOWED_DIRS` includes `workflows/`, so a composed services root keeps every bundle's workflow definitions.

## [0.66.0] - Unreleased

### Added

- `AgentMetadataConfig::agent_run` (`agentRun`, alias `agent_run`) and `DiskAgentConfig::agent_run`: an optional agent-runs `RunRequest` template, opaque JSON here. When set, the agent's A2A server queues each message as a headless agent run. Defaults to none.
- `NodeRole::Runner` and `NodeRole::runs_runner_services()`, true for `All` and `Runner`: a single-node deployment starts the extensions' runner services; a runner node starts only them.

### Breaking

- `NodeRole` gains `Runner`; exhaustive matches must handle it.

## [0.65.0] - 2026-10-09

### Added

- `database.pool.statement_cache_capacity` (default 100, validated 0..=1000) sets sqlx's per-connection prepared-statement cache. 0 is still accepted but leaks.

### Fixed

- A marketplace manifest writes `default_included` and `source` only when its `access:` block declares something (`MarketplaceAccess::is_declared`); an absent block leaves an existing row as the access-control plane set it.

### Breaking

- The profile `PoolConfig` gains `statement_cache_capacity: Option<usize>`; a struct literal must name it or use `..Default::default()`.

## [0.64.0] - 2026-10-08

### Added

- `retention.archived_users_days` (default 90): how long an archived user stays restorable before `database_cleanup` purges it.

## [0.63.1] - 2026-10-08

### Fixed

- `config::stable_instance_id` falls back to `FLY_MACHINE_ID` when `HOSTNAME` is absent or blank; Fly does not export `HOSTNAME` to application processes.

## [0.63.0] - 2026-10-07

### Added

- First release. The services manifest (`services`), startup validators (`validators`), on-disk profile (`profile`), global `Config` (`config`), secrets document (`secrets`), path constants (`paths`) and `${VAR}` interpolation (`env`) move here from `systemprompt-models`, with the `/v1/bridge/profile` builder (`bridge_profile`, formerly `systemprompt_models::bridge::profile::{build, BridgeProfileParams, provider_health, is_model_servable}`) and `McpDeploymentProvider` (formerly `systemprompt_models::mcp`). The root re-exports (`Config`, `Profile`, `Secrets`, `ServicesConfig`, `AgentConfig`, `PluginConfig`, `SkillsConfig`, `interpolate`, `split_frontmatter`, …) keep their names. Migrate by depending on `systemprompt-manifest` and renaming `systemprompt_models::{services, validators, profile, config, paths, env, secrets}` to `systemprompt_manifest::…`.
- `services::providers::surface_for(protocol)` replaces `WireProtocol::surface`.
- `ServerConfig` and `Config` gain `role: NodeRole` (`profile::NodeRole`: `all` | `gateway` | `admin`, default `all`) and `max_in_flight: Option<NonZeroU32>` (`0` rejected at parse); a struct literal must name them (`NodeRole::All` / `None` keep the old behaviour).
- `ProfileDatabaseConfig` gains `migrate_on_boot` (default `true`); a struct literal must name it.
- `StorageConfig` gains `bucket`, `prefix`, `public_read` and `credentials` and is no longer `Copy`; `StorageBackend` gains `Gcs`. Profile validation checks the bucket name and prefix, refuses `storage.shared` with `gcs` and the GCS keys with `local`. A struct literal must name the new fields (`..StorageConfig::default()`), and an exhaustive `match` on `StorageBackend` needs a `Gcs` arm.
- `GatewayConfig` gains `require_scopes: Vec<ScopeDimension>`.
- Path constants `EXPORTS`, `DATA` and `SCRATCH` (`data/scratch`) for the writable storage roots.

### Breaking

Relative to the same types in `systemprompt-models` 0.62:

- `gateway.routes[].fallback_provider` and `fallback_upstream_model` are removed; a route lists `fallbacks: [{provider, upstream_model}]` in the order they are tried, and an old key is refused at load with `unknown field`. `GatewayRoute` gains `fallbacks: Vec<RouteDeployment>` and `by_scope: Option<RouteScopeChains>`; `GatewayRoute::fallback_view` is replaced by `chain_for`, `chain_views`, `deployment_views` and `all_chains`, and `GatewayProfileError::{RouteFallbackProviderNotInRegistry, RouteFallbackIsPrimary, RouteFallbackModelWithoutProvider}` by `RouteDeploymentProviderNotInRegistry`, `RouteScopeChainProviderNotInRegistry`, `RouteDeploymentDuplicate`, `RouteScopeChainsEmpty` and `RouteScopeKeyEmpty`. Migrate `fallback_provider: x` / `fallback_upstream_model: m` to `fallbacks: [{provider: x, upstream_model: m}]`.
- `GatewayRoute.id` is `Option<RouteId>` (no empty-string sentinel); read it through `effective_id()`, and the loader backfills with `ensure_id()`.
- `ModelConfig.id` and `ModelConfig::new` use `ModelId`; `GatewayProfileError::DuplicateRouteId.id` is `RouteId`; `ProviderRegistryError::DuplicateModel.id` is `ModelId`, `EmptyModelId` carries `provider: ProviderId` and `InvalidVertexRateCard` is `{ entry: ModelId, defect: VertexRateCardDefect }`.
- `ExtensionsConfig.disabled` is `Vec<ExtensionId>`, `is_disabled` takes `&ExtensionId` and `JobConfig.extension` is `Option<ExtensionId>`.
- `TeamsAppConfig.app_id` is `TeamsAppId` and an empty `app_id` is rejected.
- `McpDeploymentProvider` is a native async trait and its `Dyn*` alias is removed.
- Every services `validate()` returns `systemprompt_models::errors::ServicesValidationError` (formerly `ConfigValidationError`), and secrets loading fails with the typed `systemprompt_models::errors::SecretsError` (`Parse { context, source }`, `PepperTooShort { min, actual }`).
- `services::skill_frontmatter::{PLATFORM_OWNED_SKILL_KEYS, is_platform_owned_skill_key, render_passthrough_frontmatter}` are `systemprompt_models::bridge::manifest::skill_frontmatter::*`; the parse, split and check helpers stay here.
- `DiskHookConfig` gains `timeout: Option<u32>` and its `id` is `Option<HookId>` (a blank id falls back to the directory name); `DiskSkillConfig` gains `frontmatter: Option<serde_yaml::Mapping>`, its id is `Option<SkillId>` resolved by `resolved_id`, and an unknown host in its `hosts` list is a validation error. `MarketplaceConfig` gains `external_plugins: Vec<ExternalPluginEntry>` and `claude_code: Option<ClaudeCodeMarketplaceConfig>`, and `ExternalMarketplaceSource::{Github, Git}` gain `reference: Option<String>` (serialised as `ref`). A struct literal must name the new fields (`None` / `vec![]` keep the old behaviour).
- `services::registry::{ServiceModule, ServiceStatus}` type the services registry (the agent and MCP status duplicates collapse onto them).
- `Profile::from_env` and the `MissingEnvVar`/`InvalidEnvVar` variants only it produced are removed.
- Cloud profiles reject `server.instance_id`; the replica identity comes from `HOSTNAME`. Migrate by deleting the key from cloud profiles.

### Fixed

- A conditional provider route is validated against the model its provider serves (id, alias or upstream name), and every deployment of every chain is priced and governance-checked at load.
