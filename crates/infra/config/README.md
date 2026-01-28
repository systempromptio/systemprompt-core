<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-config

Configuration module for systemprompt.io - environment configuration and validation.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-config.svg)](https://crates.io/crates/systemprompt-config)
[![Documentation](https://docs.rs/systemprompt-config/badge.svg)](https://docs.rs/systemprompt-config)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**

This crate provides configuration management for the systemprompt.io platform:

- **YAML Configuration Loading**: Loads and merges base and environment-specific YAML files
- **Variable Resolution**: Resolves `${VAR_NAME}` and `${VAR_NAME:-default}` patterns
- **Secrets Management**: Loads `.env.secrets` files into environment
- **Validation**: Validates configuration completeness, URL formats, and environment-specific rules
- **File Generation**: Writes `.env` files for deployment

## File Structure

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

## Modules

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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-config = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
