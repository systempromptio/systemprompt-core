# Secrets Rotation Endpoint

## Endpoint
```
POST /api/v1/tenants/{tenant_id}/rotate-credentials
```

## Authentication
Requires Bearer token authentication. User must own the tenant.

## What It Does

The endpoint performs a complete credentials rotation:

### 1. Generates New Password
- Creates a cryptographically secure random password (32 characters)
- Hashes it for storage in the management database

### 2. Updates PostgreSQL User Password
```sql
ALTER USER tenant_xxxxxxxx WITH PASSWORD 'new_secure_password'
```
This immediately changes the database user's password in PostgreSQL.

### 3. Updates Fly Secret
If the tenant has a Fly.io app deployed:
```
DATABASE_URL=postgresql://tenant_xxx:NEW_PASSWORD@host:5432/site_xxx
```
The endpoint updates the `DATABASE_URL` secret on the Fly app.

### 4. Stores Password Hash
Updates the `password_hash` column in `management.tenants` table.

## Response
```json
{
  "status": "rotated",
  "message": "Credentials rotated. Retrieve via: fly secrets list -a sp-xxxxxxxx"
}
```

## Important Notes

### URL Host
The new DATABASE_URL uses the **internal Fly host** (e.g., `sp-postgres-sandbox.internal`):
```
postgresql://tenant_xxx:password@sp-postgres-sandbox.internal:5432/site_xxx
```

This is correct for the Fly app (running inside Fly network). The app uses internal networking.

### External Access
If you need external database access:
1. Call `set_external_db_access` endpoint to enable external access
2. Use external host: `db-sandbox.systemprompt.io` (sandbox) or `db.systemprompt.io` (prod)
3. Add your IP to allowed list

### Getting the New Password
After rotation, the password is stored in Fly secrets. To retrieve it:

**Option 1: Fly CLI**
```bash
fly secrets list -a sp-xxxxxxxx
# Shows secret names but not values

# SSH into machine to see env vars
fly ssh console -a sp-xxxxxxxx
echo $DATABASE_URL
```

**Option 2: Re-provision** (not recommended)
Re-provisioning generates new secrets accessible via one-time URL.

## CLI Usage (Future)

```bash
# Rotate credentials for a tenant
systemprompt cloud tenant rotate-credentials <tenant-id>

# This should:
# 1. Call POST /api/v1/tenants/{id}/rotate-credentials
# 2. SSH into Fly app to get new DATABASE_URL
# 3. Update local tenant store with new credentials
# 4. Print connection info
```

## Code Flow

### API Handler
`crates/management-api/src/api/tenants/lifecycle.rs:rotate_credentials`
1. Verify tenant ownership
2. Call `db::rotate_tenant_credentials()` - generates password, updates PostgreSQL
3. Build new DATABASE_URL with internal host
4. Update Fly secret via `fly_provisioner.client().set_secret()`

### Database Layer
`crates/management-api/src/db/tenants/lifecycle.rs:rotate_tenant_credentials`
1. Get current tenant
2. Generate new password (32 chars, secure random)
3. Hash for storage
4. Execute `ALTER USER ... WITH PASSWORD`
5. Update tenant record with new password_hash

## Security Considerations

1. **Old credentials immediately invalidated** - ALTER USER takes effect immediately
2. **Fly app restart required** - App must restart to pick up new DATABASE_URL secret
3. **One-way operation** - Old password cannot be recovered
4. **Audit logged** - Rotation is logged with tenant_id

## Example Request

```bash
curl -X POST \
  https://api.systemprompt.io/api/v1/tenants/7dd06cd6-e2cf-4xxx-xxxx-xxxxxxxxxxxx/rotate-credentials \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json"
```

## Combining with External Access

To get a usable external DATABASE_URL after rotation:

1. **Rotate credentials**
   ```bash
   curl -X POST .../tenants/{id}/rotate-credentials
   ```

2. **Enable external access** (if not already)
   ```bash
   curl -X POST .../tenants/{id}/external-db-access \
     -d '{"enabled": true}'
   ```

3. **Get credentials from Fly**
   ```bash
   fly ssh console -a sp-xxxxxxxx
   echo $DATABASE_URL
   # postgresql://tenant_xxx:REAL_PASSWORD@sp-postgres-sandbox.internal:5432/site_xxx
   ```

4. **Swap host for external use**
   ```
   Internal: sp-postgres-sandbox.internal:5432
   External: db-sandbox.systemprompt.io:5432 (+ sslmode=require)
   ```

5. **Final external URL**
   ```
   postgresql://tenant_xxx:REAL_PASSWORD@db-sandbox.systemprompt.io:5432/site_xxx?sslmode=require
   ```
