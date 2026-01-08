# CLI Database URL Bug: External Access Flow

## Summary
When enabling external database access during tenant creation, the CLI incorrectly overwrites the real database URL (with password) with a masked URL from the API response, resulting in a non-functional stored credential.

## Evidence

| Tenant            | External Access  | Database URL Password                 |
|-------------------|------------------|---------------------------------------|
| groot2 (7dd06cd6) | true (succeeded) | `********` MASKED - BUG!              |
| groot3 (92255433) | false (failed)   | `Mp5gAAboTikBxM5...` REAL - Correct   |

**Key insight**: When external access **succeeds**, the password gets masked. When it **fails**, the real password is preserved. This proves the bug is in the success path where `database_url = response.database_url` overwrites the real URL.

## Requirements
1. Store the real database URL with password (not masked)
2. Print the external database URL with real credentials in CLI output so users can connect

## The Bug

### Code Location
`/var/www/html/systemprompt-core/crates/entry/cli/src/cloud/tenant_ops/create.rs` lines 369-388

### Current Problematic Code
```rust
let external_db_access = if enable_external {
    let spinner = CliService::spinner("Enabling external database access...");
    match client.set_external_db_access(&result.tenant_id, true).await {
        Ok(response) => {
            database_url = response.database_url;  // <-- BUG: overwrites real URL with masked URL
            spinner.finish_and_clear();
            CliService::success("External database access enabled");
            true
        },
        // ...
    }
} else {
    // ...
};
```

## Why This Is Wrong

### The Flow
1. **Provisioning**: Tenant secrets (including `database_url` with real password) are stored in `tenant_secrets` table
2. **CLI fetches secrets** (lines 331-350): Calls `/credentials/{token}` endpoint which returns secrets with real password, then **DELETES them** (one-time view security pattern)
3. **CLI has real database_url**: At this point, `database_url` variable contains the real URL with password
4. **CLI enables external access** (lines 369-388): Calls `set_external_db_access`
5. **BUG**: CLI overwrites `database_url` with `response.database_url` from the API
6. **API returns masked URL**: The API intentionally returns a masked URL (password replaced with `********`) because:
   - Secrets were already deleted after step 2
   - Even if not deleted, exposing passwords in API responses is a security concern
7. **Result**: CLI stores masked URL like `postgresql://tenant_xxx:********@db.systemprompt.io:5432/site_xxx`

### Why the API Cannot Return Real Password
The `set_external_db_access` endpoint cannot return the real database URL because:
1. **Secrets are deleted**: After the CLI calls `/credentials/{token}`, the secrets are deleted from `tenant_secrets` table (one-time view pattern)
2. **Fly secrets are write-only**: The password was set as a Fly.io secret during provisioning, but Fly secrets cannot be read back (by design)
3. **Security**: Even if available, returning passwords in API responses is bad practice

## The Fix

### Option 1: Don't Overwrite (Recommended)
The CLI should NOT overwrite the database URL from the API response. The API response is for confirmation only.

```rust
let external_db_access = if enable_external {
    let spinner = CliService::spinner("Enabling external database access...");
    match client.set_external_db_access(&result.tenant_id, true).await {
        Ok(response) => {
            // Update only the host portion, keep the password from original URL
            if let Some(ref mut url) = database_url {
                *url = update_database_url_host(url, response.external_db_access);
            }
            spinner.finish_and_clear();
            CliService::success("External database access enabled");
            true
        },
        // ...
    }
} else {
    // ...
};
```

### Option 2: Swap Host Only
If external access is enabled, swap the host from internal to external while preserving the password:

```rust
fn update_database_url_host(url: &str, external: bool) -> String {
    // Parse URL, change host based on external flag, keep password
    // Internal: {postgres_app}.internal
    // External: db.systemprompt.io (or configured external host)
}
```

### Option 3: Simplest Fix
Just don't use the response URL at all - the flag change is what matters:

```rust
let external_db_access = if enable_external {
    let spinner = CliService::spinner("Enabling external database access...");
    match client.set_external_db_access(&result.tenant_id, true).await {
        Ok(_response) => {
            // Don't touch database_url - we already have the real one
            // Just update the host portion for external access
            if let Some(ref mut url) = database_url {
                *url = swap_to_external_host(url);
            }
            spinner.finish_and_clear();
            CliService::success("External database access enabled");
            true
        },
        // ...
    }
} else {
    // ...
};

fn swap_to_external_host(url: &str) -> String {
    // postgresql://user:pass@old-host:5432/db?sslmode=disable
    // → postgresql://user:pass@db.systemprompt.io:5432/db?sslmode=require
    url.replace(".internal:5432", ".systemprompt.io:5432")
       .replace("sslmode=disable", "sslmode=require")
}
```

## What Happened in systemprompt-db

### Attempted Fix (Caused New Error)
I tried to fix this in the API by having `set_external_db_access` return the real URL:
```rust
let secrets = db::get_tenant_secrets_internal(&state.db, id).await?;
let password = extract_password_from_url(&secrets.database_url)?;
// Build real URL with password...
```

### Why It Failed
This caused: `"Tenant secrets not found. Re-provision tenant."`

Because the secrets are **already deleted** by the time `set_external_db_access` is called. The one-time secret viewing pattern means:
1. `/credentials/{token}` returns secrets
2. `/credentials/{token}` deletes secrets
3. `set_external_db_access` cannot read secrets - they're gone

## Correct Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLI Flow                                  │
├─────────────────────────────────────────────────────────────────┤
│ 1. fetch_secrets()  →  Gets real database_url with password     │
│    (API deletes secrets after returning)                        │
│                                                                  │
│ 2. database_url = secrets.database_url  ← REAL URL WITH PASSWORD│
│                                                                  │
│ 3. set_external_db_access(true)  →  API returns masked URL      │
│    (For display/confirmation only)                              │
│                                                                  │
│ 4. DON'T overwrite database_url!                                │
│    Instead: swap host from internal → external                  │
│             swap sslmode from disable → require                 │
│                                                                  │
│ 5. stored_tenant.database_url = database_url  ← Still has pass  │
└─────────────────────────────────────────────────────────────────┘
```

## Files to Modify

1. `/var/www/html/systemprompt-core/crates/entry/cli/src/cloud/tenant_ops/create.rs`
   - Lines 369-388: Don't overwrite `database_url` from response
   - Add helper to swap host for external access

2. `/var/www/html/systemprompt-db/crates/management-api/src/api/tenants/handlers.rs`
   - Revert the `get_tenant_secrets_internal` change
   - Return to using masked URL (which is correct behavior)

## Why External Access Failed for groot3

The `set_external_db_access` endpoint failed with:
```
bad_request: Tenant secrets not found. Re-provision tenant.
```

**Cause**: My attempted fix in systemprompt-db tried to read the password from `tenant_secrets`:
```rust
let secrets = db::get_tenant_secrets_internal(&state.db, id).await?;
```

But secrets were already deleted when CLI called `/credentials/{token}`. The one-time secret pattern means:
1. CLI fetches secrets → secrets deleted
2. CLI calls `set_external_db_access` → tries to read deleted secrets → FAIL

**Ironic result**: The failure preserved the real password! Because the error happened before `database_url = response.database_url` could execute.

## Required CLI Output

After enabling external access, CLI should print the connection info:
```
✓ External database access enabled

Database Connection:
  Host:     db.systemprompt.io
  Port:     5432
  Database: site_7dd06cd6e2cf
  User:     tenant_7dd06cd6e2cf
  Password: <real-password>
  SSL:      required

Connection URL:
  postgresql://tenant_7dd06cd6e2cf:<password>@db.systemprompt.io:5432/site_7dd06cd6e2cf?sslmode=require

Connect with psql:
  PGPASSWORD='<password>' psql -h db.systemprompt.io -p 5432 -U tenant_7dd06cd6e2cf -d site_7dd06cd6e2cf
```

This requires the CLI to have the real password - which it does from `fetch_secrets()`. Just don't overwrite it!

## Testing
After fix:
```bash
just tenant create
# Enable external database access: yes
# Verify:
#   1. Stored tenant has real password, not ********
#   2. CLI prints connection info with real credentials
#   3. Can actually connect: psql -h db.systemprompt.io ...
```
