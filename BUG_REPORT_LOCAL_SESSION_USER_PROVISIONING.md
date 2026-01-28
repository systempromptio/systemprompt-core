# Bug Report: Local Session Fails When Cloud User Not in Local DB

**Component:** `systemprompt-cli` (session management)
**Severity:** Medium
**Reported:** 2026-01-28

---

## Summary

When using a local profile with cloud credentials, `get_or_create_session` fails if the cloud user email doesn't exist in the local database. The system should automatically provision the user.

---

## Current Behavior

1. User authenticates with cloud credentials (`systemprompt cloud auth login`)
2. Cloud credentials stored in `.systemprompt/credentials.json` with `user_email`
3. User creates local session with `systemprompt admin session login --email <local-email>`
4. Session saved to `.systemprompt/sessions/index.json` with the local email
5. **Bug:** When running commands like `systemprompt plugins mcp call`, the CLI:
   - Reads email from `credentials.json` (cloud email), NOT from session
   - Tries to look up cloud email in local database
   - Fails with: `User 'cloud@email.com' not found in database`

```
Error: Failed to execute MCP tool

Caused by:
    User 'ed@tyingshoelaces.com' not found in database.

    Run 'systemprompt cloud auth login' to sync your user.
```

---

## Expected Behavior

For local profiles, when the cloud credentials email is not found in the local database:

1. **Auto-provision the user** in the local database with admin role
2. Continue with the session creation
3. Log: `"Auto-provisioned cloud user 'email@example.com' in local database"`

This enables seamless local development without manual user creation.

---

## Root Cause

**File:** `crates/entry/cli/src/session/creation.rs`
**Function:** `create_local_session` (lines 133-208)

```rust
// Lines 143-154: Always requires cloud credentials for local profiles
CredentialsBootstrap::try_init().await?;
let creds = CredentialsBootstrap::require()?;
let user_email = &creds.user_email;  // <-- Uses cloud email

// Line 164: Fails if user doesn't exist
let admin_user = fetch_admin(&db_pool, user_email, AdminLookupContext::Local).await?;
```

The `fetch_admin` function returns an error if the user doesn't exist, rather than creating them.

---

## Suggested Fix

In `create_local_session`, replace the strict lookup with auto-provisioning:

```rust
async fn get_or_create_local_admin(
    db_pool: &DbPool,
    email: &str,
) -> Result<systemprompt_users::User> {
    let user_service = UserService::new(db_pool)?;

    // Try to find existing user
    if let Some(user) = user_service.find_by_email(email).await? {
        if user.is_admin() {
            return Ok(user);
        }
        // Promote to admin if exists but not admin
        user_service.assign_role(&user.id, "admin").await?;
        return user_service.find_by_email(email).await?
            .ok_or_else(|| anyhow!("User disappeared after role assignment"));
    }

    // Auto-provision new user with admin role
    tracing::info!(
        email = %email,
        "Auto-provisioning cloud user in local database"
    );

    let user_id = user_service.create_user(&CreateUserRequest {
        email: email.to_string(),
        name: email.split('@').next().unwrap_or("admin").to_string(),
        ..Default::default()
    }).await?;

    user_service.assign_role(&user_id, "admin").await?;

    user_service.find_by_id(&user_id).await?
        .ok_or_else(|| anyhow!("Failed to fetch newly created user"))
}
```

Then update `create_local_session`:

```rust
// Replace:
let admin_user = fetch_admin(&db_pool, user_email, AdminLookupContext::Local).await?;

// With:
let admin_user = get_or_create_local_admin(&db_pool, user_email).await?;
```

---

## Workaround

Manually create the user matching cloud credentials:

```bash
# Get cloud email
cat .systemprompt/credentials.json | jq -r '.user_email'

# Create user in local database
systemprompt admin users create --name admin --email <cloud-email>

# Get user ID from output, then assign admin role
systemprompt admin users role assign --roles admin <user-id>
```

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/entry/cli/src/session/creation.rs` | Add `get_or_create_local_admin` function |
| `crates/entry/cli/src/session/creation.rs` | Update `create_local_session` to use it |

---

## Test Cases

1. **New local setup with cloud auth**: Cloud user auto-provisioned on first local command
2. **Existing non-admin user**: Promoted to admin automatically
3. **Existing admin user**: No change, normal flow
4. **No cloud credentials**: Error with instructions to run cloud auth

---

## Related

- Session storage: `.systemprompt/sessions/index.json`
- Cloud credentials: `.systemprompt/credentials.json`
- User service: `crates/domain/users/`
