# systemprompt-traits Unit Tests

## Crate Overview
Minimal shared trait contracts for architecture boundaries including database value traits and startup event traits.

## Source Files
- `src/lib.rs` - Trait exports
- `src/db_value.rs` - DbValue, ToDbValue traits
- `src/startup_events.rs` - Application lifecycle traits

## Test Plan

### DbValue Trait Tests
- `test_to_db_value_string` - String conversion
- `test_to_db_value_uuid` - UUID conversion
- `test_to_db_value_integer` - Integer conversion
- `test_to_db_value_optional` - Option<T> handling
- `test_from_database_row` - Row extraction

### Startup Events Tests
- `test_startup_event_trait_impl` - Trait implementation verification

## Mocking Requirements
- Mock database rows for FromDatabaseRow tests

## Test Fixtures Needed
- Sample database row data
