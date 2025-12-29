# systemprompt-runtime Unit Tests

## Crate Overview
Application runtime context and module registry. Handles module registration, API registration, and startup validation.

## Source Files
- `src/context/` - AppContext, AppContextBuilder
- `src/installation/` - Module installation
- `src/registry/` - ModuleApiRegistry
- `src/span/` - Request span creation
- `src/startup_validation/` - StartupValidator
- `src/validation/` - System validation
- `src/wellknown/` - Well-known metadata

## Test Plan

### AppContext Tests
- `test_context_builder` - Build context
- `test_context_initialization` - Initialize context
- `test_context_module_access` - Access modules

### Module Registration Tests
- `test_module_install` - Install module
- `test_module_install_with_db` - Install with DB
- `test_module_registration` - Register module

### API Registry Tests
- `test_api_registration` - Register API
- `test_api_lookup` - Lookup API
- `test_api_list` - List APIs

### Startup Validation Tests
- `test_startup_validation_pass` - Validation passes
- `test_startup_validation_fail` - Validation fails
- `test_system_validation` - System check

### Well-known Tests
- `test_wellknown_metadata` - Get metadata
- `test_wellknown_route_registration` - Register route

## Mocking Requirements
- Mock database
- Mock modules

## Test Fixtures Needed
- Sample module configs
