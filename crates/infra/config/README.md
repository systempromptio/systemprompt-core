<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-config

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-config — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-config.svg?style=flat-square)](https://crates.io/crates/systemprompt-config)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-config?style=flat-square)](https://docs.rs/systemprompt-config)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

One profile is the source of truth for how your deployment runs. Not a scatter of environment variables nobody audited. This crate loads that profile and its secrets, then installs both before any other layer starts.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

Configuration comes from the active profile, read once at boot. Environment variables are a scoped escape hatch (`${VAR}` interpolation inside profile YAML and a small set of sanctioned overrides), never a general fallback, so what governs the process is auditable in one file you own.

The crate loads the profile YAML, reads the secrets document it references, and installs both into process-wide singletons in a fixed order: profile before secrets. It also backs the deployment pipeline (`systemprompt cloud config`) and the `admin config` CLI surfaces that mutate a profile's provider registry and security section.

## Modules

| Module | Purpose |
|--------|---------|
| `bootstrap` | Process-wide cells for the active profile and secrets (`ProfileBootstrap`, `SecretsBootstrap`), plus the manifest-signing seed helpers. Ordering (profile before secrets) is a runtime invariant of the entry-crate boot sequence. |
| `config_loader` | Builds a runtime `Config` from the active profile (`init_config`, `try_init_config`, `build_from_profile`); `validate_database_config` checks database wiring before startup. |
| `profile_loader` | `load_profile_with_catalog` — profile YAML parsing with optional catalog overlay. |
| `profile_gateway` | Profile lookup gateway used during routing resolution. |
| `path_validation` | Validates the filesystem paths a profile declares (`validate_profile_paths`) and formats path-error reports. |
| `services` | Deployment and admin utilities: `ConfigService` (in `services/service.rs`), `ConfigValidator`, `ProviderCatalogService`, `SecurityConfigService`, and the schema-validation helpers. |
| `skill_validator` | `SkillConfigValidator` walks the `skills/` tree and reports missing or malformed manifests through the `DomainConfig` trait. |

`ConfigService` lives in `services/service.rs`. `ProviderCatalogService` (typed mutations of the profile's provider registry, backing `admin config catalog`) and `SecurityConfigService` with `SecurityUpdate` / `SecurityChange` (backing `admin config security`) live in `services/provider_catalog.rs` and `services/security_config.rs`.

## Usage

```toml
[dependencies]
systemprompt-config = "0.21"
```

```rust
use systemprompt_config::{
    ConfigResult, ProfileBootstrap, SecretsBootstrap, init_config_from_profile,
};

fn boot() -> ConfigResult<()> {
    let profile = ProfileBootstrap::init()?;
    SecretsBootstrap::init()?;
    let config = init_config_from_profile(profile)?;
    let _ = config;
    Ok(())
}
```

## Public API

```rust
use systemprompt_config::{
    // Bootstrap
    ProfileBootstrap, ProfileBootstrapError,
    SecretsBootstrap, SecretsBootstrapError,
    build_loaded_secrets_message, load_secrets_from_path,
    log_secrets_issue, log_secrets_skip, log_secrets_warn,
    decode_seed, generate_seed, persist_seed,
    MANIFEST_SIGNING_SEED_BYTES,

    // Runtime config
    build_from_profile, init_config, init_config_from_profile,
    try_init_config, validate_database_config,
    load_profile_with_catalog,

    // Errors
    ConfigError, ConfigResult,

    // Deployment services
    ConfigService, ConfigValidator, ConfigValidationError,
    DeployEnvironment, DeploymentConfig, EnvironmentConfig,
    ValidationReport,
    generate_schema, validate_config, validate_yaml_file, validate_yaml_str,

    // Admin config services
    ProviderCatalogService, ProviderSpec, ModelSpec,
    SecurityConfigService, SecurityUpdate, SecurityChange,

    // Skill validation
    SkillConfigValidator,
};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | `Config` and profile/secrets data types |
| `systemprompt-traits` | `DomainConfig` trait implemented by `SkillConfigValidator` |
| `systemprompt-identifiers` | Typed identifiers used across profile and secrets types |
| `systemprompt-logging` | CLI output via `CliService` |
| `serde`, `serde_json`, `serde_yaml` | Profile, secrets, and config serialisation |
| `schemars` | JSON schema generation |
| `regex` | `${VAR}` and `${VAR:-default}` resolution |
| `base64`, `rand` | Manifest signing seed encoding |
| `thiserror` | `ConfigError` and downstream typed errors |
| `tracing` | Structured logging during bootstrap |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-config)** · **[docs.rs](https://docs.rs/systemprompt-config)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
