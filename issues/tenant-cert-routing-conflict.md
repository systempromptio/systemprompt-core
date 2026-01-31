# Tenant Certificate Routing Conflict - Post-Mortem

## Summary

After deployment, tenant site `999bc6549a64.systemprompt.io` became unreachable with SSL handshake errors. Root cause: a certificate was automatically added to the tenant app instead of the Management API, causing routing conflicts at the edge proxy.

## Environment

- Tenant: `999bc654-9a64-49bc-98be-db976fc84e76`
- Tenant App: `sp-999bc6549a64`
- Date: 2026-01-31
- Profile: `systemprompt-prod`

## Timeline

1. **06:20** - Deployment initiated via `systemprompt cloud deploy`
2. **06:28** - Deployment completed successfully, logs show app started
3. **06:28** - Site unreachable, SSL handshake fails with "unexpected eof while reading"
4. **06:35** - Root cause identified: conflicting certificate on tenant app
5. **06:35** - Fix applied: removed cert from tenant, added to Management API
6. **06:35** - Site accessible again

## Symptoms

```bash
$ curl -v https://999bc6549a64.systemprompt.io/
*   Trying 66.241.124.121:443...
* Connected to 999bc6549a64.systemprompt.io (66.241.124.121) port 443 (#0)
* TLSv1.3 (OUT), TLS handshake, Client hello (1):
* TLSv1.3 (OUT), TLS alert, decode error (562):
* error:0A000126:SSL routines::unexpected eof while reading
curl: (35) error:0A000126:SSL routines::unexpected eof while reading
```

Meanwhile, direct access to tenant app worked:
```bash
$ curl -sI https://sp-999bc6549a64.fly.dev/
HTTP/2 200
```

And other wildcard subdomains worked:
```bash
$ curl -sI https://test12345.systemprompt.io/
HTTP/2 502  # Expected - no such tenant, but SSL worked
```

## Root Cause Analysis

### Architecture Context

```
*.systemprompt.io (DNS) → Management API → sp-{tenant-id} (tenant app)
                              ↓
                    Wildcard SSL cert here
```

The Management API holds the wildcard certificate (`*.systemprompt.io`) and routes requests to tenant apps. Tenant apps should **never** have certificates for `*.systemprompt.io` subdomains.

### What Happened

A certificate for `999bc6549a64.systemprompt.io` was added to the **tenant app** (`sp-999bc6549a64`) instead of the Management API:

```bash
$ fly certs list -a sp-999bc6549a64
Host Name                      Status
999bc6549a64.systemprompt.io   Awaiting configuration
```

This created a conflict:
1. DNS points to Management API (correct)
2. Management API has wildcard cert (correct)
3. Tenant app **also** claims the hostname via its cert (incorrect)
4. Edge proxy doesn't know which app should terminate SSL
5. Connection drops during TLS handshake

### How Did This Certificate Get Added?

**This is the critical question.** The tenant site was freshly deployed and had been working. Possible causes:

1. **Deployment pipeline bug** - Does `cloud deploy` or a post-deploy hook automatically add certs to tenant apps?
2. **Tenant provisioning bug** - When tenant was created, did it incorrectly add a cert?
3. **Management API auto-cert logic** - Is there code that adds certs to the wrong target?
4. **Manual error** - Someone ran the wrong command (unlikely for fresh site)

## Investigation Required

### Check Deployment Pipeline

```rust
// Look for certificate-related code in deploy handlers
// File: crates/management-api/src/api/tenants/handlers.rs

// Does deploy_tenant() or any post-deploy step add certs?
// If so, is it adding to the correct app (management-api-prod)?
```

### Check Tenant Provisioning

```rust
// Look for certificate-related code in tenant creation
// File: crates/management-api/src/api/tenants/handlers.rs

// Does create_tenant() add any certificates?
// If so, which app does it target?
```

### Relevant Code Paths

Search for:
- `certs add`
- `fly certs`
- `Certificate`
- `add_certificate`
- `--hostname`

In:
- `crates/management-api/`
- Any deployment scripts or jobs

## Resolution

### Immediate Fix (Applied)

```bash
# Remove conflicting certificate from tenant app
fly certs remove 999bc6549a64.systemprompt.io -a sp-999bc6549a64 -y

# Add certificate to Management API (where it belongs)
fly certs add 999bc6549a64.systemprompt.io -a management-api-prod

# Verify
fly certs show 999bc6549a64.systemprompt.io -a management-api-prod
# Status: Ready
```

### Permanent Fix Required

1. **Find the code** that adds certificates to tenant apps
2. **Remove or fix** that logic - certs should only go to `management-api-prod`
3. **Add validation** to prevent certs from being added to `sp-*` apps for `*.systemprompt.io`
4. **Add monitoring** to detect certificate conflicts

## Recommended Changes

### Option 1: Remove Auto-Cert Logic

If certificates are being auto-added during deployment, remove that logic. The wildcard cert on Management API covers all subdomains.

### Option 2: Fix Target App

If auto-cert is needed (e.g., for custom domains), ensure it targets `management-api-prod`:

```rust
// Wrong
fly_client.add_cert(&tenant_hostname, &tenant_app_name)?;

// Correct
fly_client.add_cert(&tenant_hostname, "management-api-prod")?;
```

### Option 3: Add Guard Rails

```rust
// Prevent certs on tenant apps for systemprompt.io subdomains
fn add_certificate(hostname: &str, app: &str) -> Result<()> {
    if hostname.ends_with(".systemprompt.io") && app.starts_with("sp-") {
        return Err(Error::InvalidCertTarget(
            "*.systemprompt.io certs must be on management-api-prod"
        ));
    }
    // ... proceed
}
```

## Impact

- **Severity**: High (site completely unreachable)
- **Duration**: ~15 minutes
- **Affected**: Single tenant
- **Data Loss**: None

## Lessons Learned

1. SSL certificate placement is critical in multi-tenant proxy architectures
2. The wildcard cert on Management API should handle all subdomains
3. Individual tenant apps should never have `*.systemprompt.io` certs
4. Need better observability for certificate state across apps

## Action Items

- [ ] Find where certificate was added to tenant app (code review)
- [ ] Fix the deployment/provisioning logic
- [ ] Add validation to prevent future conflicts
- [ ] Document certificate architecture in playbooks
- [ ] Consider adding health check for certificate conflicts
