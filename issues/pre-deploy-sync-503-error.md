# Pre-Deploy Sync 503 Error

## Summary

When running `cloud deploy` with a cloud profile, the pre-deploy sync step fails with a 503 API error, preventing deployment unless `--no-sync` is used.

## Environment

- Profile: `systemprompt-prod`
- Tenant: `999bc6549a64`
- Date: 2026-01-28

## Reproduction Steps

1. Ensure you have a valid cloud profile configured:
   ```bash
   systemprompt admin session switch systemprompt-prod
   ```

2. Verify cloud authentication:
   ```bash
   systemprompt cloud auth whoami
   ```

3. Attempt to deploy:
   ```bash
   just deploy --profile systemprompt-prod
   ```

4. Observe the error during pre-deploy sync phase.

## Expected Behavior

The pre-deploy sync should either:
- Complete successfully and download runtime files from the container
- Fail gracefully with a clear error message indicating the cause

## Actual Behavior

The sync fails with a 503 error:

```
systemprompt.io Cloud Deploy

Pre-Deploy Sync
⚠ DESTRUCTIVE OPERATION
ℹ   Deploying replaces the running container.
ℹ   Runtime files (uploads, AI-generated images) not in your local build
ℹ   will be PERMANENTLY LOST unless synced first.
ℹ
ℹ   Database records are preserved.
ℹ
✗ Sync error: API error 503:

Error: Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).
```

## Workaround

Use `--no-sync` flag to skip the sync step:

```bash
systemprompt cloud deploy --profile systemprompt-prod --no-sync
```

**Warning**: This may result in loss of runtime files (uploads, generated images) that exist only on the container.

## Impact

- **Severity**: Medium
- **Frequency**: Consistent on affected tenant
- **User Impact**: Requires workaround to deploy; potential data loss if runtime files exist

## Possible Causes

1. The cloud tenant API endpoint may be temporarily unavailable
2. The sync endpoint may have a bug or timeout issue
3. Authentication/authorization issue with the sync operation
4. Network connectivity issue between local machine and cloud tenant

## Investigation Notes

- The 503 error suggests a server-side issue (Service Unavailable)
- The error message is incomplete (no details after "API error 503:")
- The cloud tenant was functional after deployment, suggesting the issue is specific to the sync endpoint

## Recommended Fix

1. Add better error handling to capture the full 503 response body
2. Implement retry logic for transient 503 errors
3. Add a health check before attempting sync to provide clearer error messages
4. Consider making sync optional by default with a `--sync` flag to enable it
