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

Profile-based configuration for systemprompt.io AI governance infrastructure. Bootstraps profiles, secrets, and credentials with zero environment-variable fallback. Provides configuration management including YAML loading, variable resolution, secrets management, and validation.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides configuration management for the systemprompt.io platform:

- **YAML Configuration Loading**: Loads and merges base and environment-specific YAML files
- **Variable Resolution**: Resolves `${VAR_NAME}` and `${VAR_NAME:-default}` patterns
- **Secrets Management**: Loads `.env.secrets` files into environment
- **Validation**: Validates configuration completeness, URL formats, and environment-specific rules
- **File Generation**: Writes `.env` files for deployment

## Architecture

```
src/
├── lib.rs                      # Crate root - public API exports
└── services/
    ├── mod.rs                  # Module declarations and re-exports
    ├── manager.rs              # ConfigManager - YAML loading, merging, variable resolution
    ├── schema_validation.rs    # Generic YAML/JSON schema validation utilities
    ├── types.rs                # DeployEnvironment, DeploymentConfig, EnvironmentConfig
    ├── validator.rs            # ConfigValidator, ValidationReport
    └── writer.rs               # ConfigWriter - .env file generation
```

### `manager.rs`
Core configuration management functionality:
- `ConfigManager::new(project_root)` - Initialize with project path
- `ConfigManager::generate_config(environment)` - Load and merge YAML configs
- Variable resolution with environment variable fallback
- Secrets file loading

### `schema_validation.rs`
Generic schema validation utilities:
- `validate_config<T>()` - Validate YAML against typed schema
- `validate_yaml_file()` - Parse and validate YAML syntax
- `generate_schema<T>()` - Generate JSON schema from types
- `build_validate_configs()` - Build-time validation for build.rs

### `types.rs`
Configuration type definitions:
- `DeployEnvironment` - Enum: Local, DockerDev, Production
- `DeploymentConfig` - Raw YAML configuration container
- `EnvironmentConfig` - Resolved environment variables

### `validator.rs`
Configuration validation:
- `ConfigValidator::validate()` - Run all validation checks
- `ValidationReport` - Collect errors and warnings
- Checks: unresolved variables, required variables, URL formats, port values

### `writer.rs`
Configuration file output:
- `ConfigWriter::write_env_file()` - Write standard .env file
- `ConfigWriter::write_web_env_file()` - Write VITE_* variables for web builds

## Usage

```toml
[dependencies]
systemprompt-config = "0.2.1"
```

```rust
use std::path::PathBuf;
use systemprompt_config::{ConfigManager, ConfigValidator, DeployEnvironment};

fn load_env(project_root: PathBuf) -> anyhow::Result<()> {
    let manager = ConfigManager::new(project_root);
    let config = manager.generate_config(DeployEnvironment::Local)?;
    let report = ConfigValidator::validate(&config);
    report.into_result()?;
    Ok(())
}
```

## Public API

```rust
use systemprompt_config::{
    ConfigManager,
    ConfigValidator,
    DeployEnvironment,
    EnvironmentConfig,
    ValidationReport,
    validate_config,
    validate_yaml_file,
};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-logging` | CLI output via CliService |
| `serde_yaml` | YAML parsing |
| `schemars` | JSON schema generation |
| `regex` | Variable resolution patterns |
| `anyhow` | Error handling |
| `thiserror` | Typed errors for schema validation |
| `tracing` | Warning logs for unsupported features |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-config)** · **[docs.rs](https://docs.rs/systemprompt-config)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
