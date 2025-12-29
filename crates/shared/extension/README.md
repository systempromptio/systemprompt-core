# systemprompt-extension

Extension framework for SystemPrompt - register custom modules, providers, and APIs.

## Purpose

Provides the infrastructure for building and loading SystemPrompt extensions.
Extensions can add new routes, services, and capabilities to the platform.

## Key Types

- `ExtensionContext` - Runtime context for extensions
- `ExtensionError` - Error types for extension operations
- `ExtensionLoader` - Registration and loading system

## Dependencies

- `async-trait` - Async trait support
- `axum` - Router types
- `tokio` - Async runtime
- `reqwest` - HTTP client
