# systemprompt-core-config

Configuration module for environment configuration management and validation.

## Structure

```
src/
├── lib.rs                  # Crate exports
└── services/
    ├── mod.rs              # Module declarations
    ├── manager.rs          # ConfigManager - YAML loading, variable resolution
    ├── types.rs            # DeployEnvironment, DeploymentConfig, EnvironmentConfig
    ├── validator.rs        # ConfigValidator, ValidationReport
    └── writer.rs           # File writing for .env files
```

## Public API

- `ConfigManager` - Loads and merges YAML configs, resolves variables
- `ConfigValidator` - Validates configuration completeness and correctness
- `DeployEnvironment` - Enum: Local, DockerDev, Production
- `DeploymentConfig` - Flattened YAML configuration
- `EnvironmentConfig` - Resolved environment variables
- `ValidationReport` - Validation errors and warnings

## Dependencies

- `systemprompt-core-logging` - CLI output via CliService
- `serde_yaml` - YAML parsing
- `regex` - Variable resolution patterns
- `anyhow` - Error handling
