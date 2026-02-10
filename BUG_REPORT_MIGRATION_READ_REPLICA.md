# Bug: Migrations fail on read-only replica when DATABASE_URL points to port 5433

## Problem

With the multi-region Postgres setup, `DATABASE_URL` now points to port 5433 (nearest read replica) and `DATABASE_WRITE_URL` points to port 5432 (primary). On startup, the tenant app runs schema migrations using `DATABASE_URL`, which hits the read-only replica and fails:

```
Error: Failed to install extension schemas
Caused by:
    cannot execute CREATE FUNCTION in a read-only transaction
```

## Root Cause

The migration/schema installation code uses `DATABASE_URL` for all database operations including DDL statements (`CREATE FUNCTION`, `CREATE TABLE`, etc.). Since `DATABASE_URL` now routes to the nearest replica (port 5433), these write operations are rejected.

## Expected Behavior

Migrations and schema setup should use `DATABASE_WRITE_URL` (port 5432, primary) since they require write access. Normal read queries should continue using `DATABASE_URL` (port 5433, nearest replica).

## Fix Required

Wherever migrations or schema installations run on startup:
- Check for `DATABASE_WRITE_URL` environment variable
- If present, use it for migrations/DDL operations
- Fall back to `DATABASE_URL` if `DATABASE_WRITE_URL` is not set (backwards compatible)

## Environment

- `DATABASE_URL` = `postgresql://...@systemprompt-db-prod.internal:5433/...` (read replica)
- `DATABASE_WRITE_URL` = `postgresql://...@systemprompt-db-prod.internal:5432/...` (primary)
- Postgres replica in `fra`, primary in `iad`
- Tenant app: `sp-e946cb5de40c` (Frankfurt)
