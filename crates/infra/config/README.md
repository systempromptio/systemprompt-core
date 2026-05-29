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
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Profile-based configuration for systemprompt.io AI governance infrastructure. Bootstraps profiles, secrets, and credentials with zero environment-variable fallback.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate is the bootstrap layer for the platform. It loads the active profile YAML, the matching secrets document, and installs both into process-wide singletons before any other layer (database, runtime, agent) starts. It also exposes the deployment-time `ConfigService` used by `systemprompt cloud config` and a `DomainConfig` validator for skill manifests.

- **Type-state bootstrap**: `BootstrapSequence` enforces *profile before secrets* at compile time.
- **Profile loading**: Parses `.systemprompt/profiles/<name>/profile.yaml`, with optional catalog overlay.
- **Secrets loading**: Reads the secrets document referenced by the active profile and seeds the in-process store.
- **Runtime config construction**: Builds a `systemprompt_models::Config` from the active profile.
- **Deployment config**: `ConfigService` resolves `${VAR}` / `${VAR:-default}` patterns and emits `.env` files for downstream services.
- **Schema validation**: Generic YAML/JSON validation utilities and a `SkillConfigValidator` for the `skills/` tree.

## Architecture

```
src/
├── lib.rs                      # Crate root — public API surface
├── error.rs                    # ConfigError / ConfigResult<T>
├── config_loader.rs            # init_config, build_from_profile, validate_database_config
├── profile_loader.rs           # load_profile_with_catalog
├── profile_gateway.rs          # Profile lookup gateway
├── skill_validator.rs          # SkillConfigValidator (DomainConfig impl)
├── bootstrap/
│   ├── mod.rs                  # BootstrapSequence, type-state markers, presets
│   ├── profile.rs              # ProfileBootstrap singleton
│   ├── manifest.rs             # Manifest signing seed helpers
│   └── secrets/
│       ├── mod.rs              # SecretsBootstrap singleton
│       ├── loader.rs           # load_secrets_from_path
│       ├── io.rs               # Disk I/O for secrets documents
│       └── logging.rs          # log_secrets_issue / skip / warn helpers
└── services/
    ├── mod.rs                  # Re-exports
    ├── manager.rs              # ConfigService — YAML loading, merging, variable resolution
    ├── report.rs               # ValidationReport
    ├── schema_validation.rs    # validate_config, validate_yaml_file, generate_schema
    ├── types.rs                # DeployEnvironment, DeploymentConfig, EnvironmentConfig
    ├── validator.rs            # ConfigValidator
    └── writer.rs               # .env file generation
```

### `bootstrap/`
Process-wide cells for the active profile and secrets document, plus the type-state `BootstrapSequence` that drives `Uninitialized → ProfileInitialized → SecretsInitialized → BootstrapComplete`. Manifest seed helpers (`generate_seed`, `decode_seed`, `persist_seed`) live alongside.

### `config_loader.rs`
Builds a runtime `Config` from the active profile via `init_config`, `try_init_config`, `init_config_from_profile`, and `build_from_profile`. `validate_database_config` checks database wiring before startup.

### `services/`
Deployment-pipeline utilities consumed by `systemprompt cloud config`: `ConfigService` loads and merges YAML, `ConfigValidator` produces a `ValidationReport`, and the schema-validation helpers operate over arbitrary `serde` types.

### `skill_validator.rs`
`SkillConfigValidator` walks the `skills/` directory and reports missing or malformed manifests through the `DomainConfig` trait.

## Usage

```toml
[dependencies]
systemprompt-config = "0.13.0"
```

```rust
use systemprompt_config::{
    presets, BootstrapSequence, ConfigResult, init_config_from_profile,
};

fn boot() -> ConfigResult<()> {
    let complete = presets::standard()?;
    let config = init_config_from_profile(complete.profile())?;
    let _ = config;
    Ok(())
}
```

## Public API

```rust
use systemprompt_config::{
    // Bootstrap
    BootstrapSequence, BootstrapComplete, BootstrapState,
    ProfileBootstrap, ProfileInitialized, ProfileBootstrapError,
    SecretsBootstrap, SecretsInitialized, SecretsBootstrapError,
    Uninitialized, presets,
    build_loaded_secrets_message, load_secrets_from_path,
    log_secrets_issue, log_secrets_skip, log_secrets_warn,
    decode_seed, generate_seed, persist_seed,
    JWT_SECRET_MIN_LENGTH, MANIFEST_SIGNING_SEED_BYTES,

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

    // Skill validation
    SkillConfigValidator,
};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | `Config` and profile/secrets data types |
| `systemprompt-traits` | `DomainConfig` trait implemented by `SkillConfigValidator` |
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
