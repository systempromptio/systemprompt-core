# systemprompt-runtime

Application runtime context and module registry. Provides centralized access to database, configuration, and extension services.

## Structure

```
src/
├── lib.rs                    # Public exports and macros
├── context.rs                # AppContext and builder
├── installation.rs           # Module installation
├── registry.rs               # Extension registry
├── span.rs                   # Tracing span helpers
├── startup_validation.rs     # Startup checks
├── validation.rs             # System validation
└── wellknown.rs              # Well-known endpoint metadata
```

## Key Components

| Component | Description |
|-----------|-------------|
| AppContext | Centralized runtime state |
| AppContextBuilder | Builder for custom configuration |
| ExtensionRegistry | Extension discovery and management |
| Validation | System prerequisite checks |

## Macros

| Macro | Purpose |
|-------|---------|
| `register_module_api!` | Register module routes with runtime |
| `register_wellknown!` | Register .well-known endpoints |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-core-database` | Database pool |
| `systemprompt-core-config` | Configuration loading |
| `systemprompt-models` | Module definitions |
| `inventory` | Compile-time registration |
