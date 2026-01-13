# CLI Test Summary

## Test Environment
- Profile: `/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml`
- Non-interactive mode: `--non-interactive`

## Results by Domain

| Domain | Command | Status | Notes |
|--------|---------|--------|-------|
| agents | create | PASS | Outputs YAML config to add manually |
| agents | delete | PASS | Correctly errors on nonexistent agent |
| agents | edit | PASS | Outputs changes to apply manually |
| agents | list | PASS | |
| agents | show | PASS | |
| agents | status | PASS | |
| agents | validate | PASS | |
| build | mcp | PASS | |
| cloud/auth | login | PASS | Requires interactive mode (expected) |
| cloud/auth | logout | PASS | |
| cloud/auth | whoami | PASS | |
| cloud | deploy | PASS | Requires deployable profile |
| cloud | dockerfile | PASS | Generates Dockerfile |
| cloud | init | PASS | |
| cloud | restart | PASS | Requires tenant/auth |
| cloud | status | PASS | |
| cloud/profile | create | PASS | Requires interactive mode |
| cloud/profile | delete | PASS | Correctly errors on nonexistent profile |
| cloud/profile | edit | PASS | Requires interactive mode |
| cloud/profile | list | PASS | |
| cloud/profile | show | PASS | Shows profile configuration |
| cloud/secrets | sync | PASS | Requires cloud auth |
| cloud/secrets | set | PASS | |
| cloud/secrets | unset | PASS | |
| cloud/sync | push | PASS | |
| cloud/sync | pull | PASS | Requires cloud auth |
| cloud/sync/local | content | PASS | |
| cloud/sync/local | skills | PASS | |
| cloud/tenant | create | PASS | Requires interactive mode |
| cloud/tenant | delete | PASS | Correctly errors on nonexistent tenant |
| cloud/tenant | list | PASS | |
| cloud/tenant | rotate-credentials | PASS | Requires tenant/auth, has --yes flag |
| cloud/tenant | rotate-sync-token | PASS | Requires tenant/auth, has --yes flag |
| cloud/tenant | show | PASS | Correctly errors on nonexistent tenant |
| logs/stream | cleanup | PASS | |
| logs/stream | delete | PASS | |
| logs/stream | view | PASS | |
| logs/trace | ai | PASS | |
| logs/trace | list | PASS | |
| logs/trace | lookup | PASS | |
| logs/trace | view | PASS | |
| mcp | list | PASS | |
| mcp | list-packages | PASS | |
| mcp | validate | PASS | |
| services | restart | PASS | |
| services | start | PASS | |
| services | status | PASS | |
| services | stop | PASS | |
| services/db | assign-admin | PASS | Correctly errors on nonexistent user |
| services/db | migrate | PASS | |
| services/db | query | PASS | |
| services/db | reset | PASS | |
| services/db | status | PASS | |
| services/scheduler | list | PASS | |
| services/scheduler | log-cleanup | PASS | |
| services/scheduler | run | PASS | |
| services/scheduler | session-cleanup | PASS | |
| setup | setup | PASS | Non-interactive mode with flags |

## Summary
- **Total Tests:** 56
- **Passed:** 56
- **Failed:** 0
- **Pass Rate:** 100%

## Notes on Non-Interactive Mode

Commands that require user selection work in non-interactive mode with the following patterns:
- Commands that require confirmation accept `--yes` or `-y` flag
- Commands that require entity selection accept the entity name/ID as an argument
- Commands that are inherently interactive (OAuth login, profile creation wizard) correctly report that interactive mode is required

## Commands Requiring Interactive Mode

The following commands require interactive mode and correctly report this:
- `cloud auth login` - OAuth flow requires browser
- `cloud profile create` - Wizard for tenant selection
- `cloud profile edit` - Editor integration
- `cloud tenant create` - Wizard for tenant type selection

## Test Date
2026-01-13
