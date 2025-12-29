# systemprompt-core-database Unit Tests

## Crate Overview
Database abstraction layer supporting SQLite, PostgreSQL, and MySQL through SQLx. Provides repository patterns, transaction management, and schema lifecycle.

## Source Files
- `src/admin/` - Database introspection and query execution
- `src/lifecycle/` - Schema installation, validation, seeds
- `src/models/` - Database value types, query results
- `src/repository/` - Repository patterns, CRUD operations
- `src/services/` - Database provider, transactions, connection pooling

## Test Plan

### Repository Tests
- `test_repository_insert_record` - Insert operation
- `test_repository_find_by_id` - Find by primary key
- `test_repository_find_all` - List all records
- `test_repository_update_record` - Update operation
- `test_repository_delete_record` - Delete operation
- `test_repository_find_with_filter` - Filtered queries

### Transaction Tests
- `test_transaction_commit_success` - Successful commit
- `test_transaction_rollback_on_error` - Rollback behavior
- `test_nested_transaction_handling` - Nested transactions

### Connection Pool Tests
- `test_pool_acquire_connection` - Connection acquisition
- `test_pool_max_connections` - Pool limits
- `test_pool_connection_timeout` - Timeout handling

### Schema Lifecycle Tests
- `test_schema_installation` - Install schema
- `test_schema_validation` - Validate schema
- `test_migration_execution` - Run migrations

## Mocking Requirements
- In-memory SQLite for testing
- Mock connection pools

## Test Fixtures Needed
- Sample migration files
- Test schema definitions
