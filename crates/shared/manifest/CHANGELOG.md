# Changelog

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
