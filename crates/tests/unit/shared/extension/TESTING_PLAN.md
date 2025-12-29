# systemprompt-extension Unit Tests

## Crate Overview
Extension framework for custom modules, providers, and APIs. Supports plugin discovery and typed extensions.

## Source Files
- `src/lib.rs` - Extension exports
- `src/typed/` - Type-safe extensions

## Test Plan

### Extension Registration Tests
- `test_register_extension_success` - Registration flow
- `test_register_duplicate_extension` - Duplicate handling
- `test_unregister_extension` - Removal flow

### Plugin Discovery Tests
- `test_discover_plugins_in_directory` - Directory scanning
- `test_plugin_validation` - Plugin format validation
- `test_plugin_loading` - Dynamic loading

### Typed Extension Tests
- `test_typed_extension_creation` - Type-safe creation
- `test_typed_extension_retrieval` - Type-safe retrieval

## Mocking Requirements
- Mock filesystem for plugin discovery

## Test Fixtures Needed
- Sample plugin files
