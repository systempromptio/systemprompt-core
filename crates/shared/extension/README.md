# systemprompt-extension

Extension framework for systemprompt.io - register custom modules, providers, and APIs.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-extension.svg)](https://crates.io/crates/systemprompt-extension)
[![Documentation](https://docs.rs/systemprompt-extension/badge.svg)](https://docs.rs/systemprompt-extension)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

**Part of the Shared layer in the systemprompt.io architecture.**

Provides the infrastructure for building and loading systemprompt.io extensions.
Extensions can add new routes, services, and capabilities to the platform.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-extension = "0.0.1"
```

## Quick Example

```rust
use systemprompt_extension::prelude::*;

struct MyExtension;

impl Extension for MyExtension {
    fn id(&self) -> &str { "my-extension" }
    fn name(&self) -> &str { "My Extension" }
    fn version(&self) -> &str { "1.0.0" }
}

register_extension!(MyExtension);
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | Yes | HTTP API routes via Axum |
| `plugin-discovery` | No | Dynamic plugin loading |

## Key Types

- `ExtensionContext` - Runtime context for extensions
- `ExtensionError` - Error types for extension operations
- `ExtensionLoader` - Registration and loading system

## Dependencies

- `async-trait` - Async trait support
- `axum` - Router types (optional, with `web` feature)
- `inventory` - Compile-time extension registration
- `reqwest` - HTTP client (optional, with `web` feature)

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
