# CLI Test Summary

## Test Environment
- Profile: `/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml`
- Non-interactive mode: `--non-interactive`

## Results by Domain

| Domain | Command | Status |
|--------|---------|--------|
| agents | create | FAIL |
| agents | delete | FAIL |
| agents | edit | FAIL |
| agents | list | PASS |
| agents | show | PASS |
| agents | status | PASS |
| agents | validate | PASS |
| build | mcp | PASS |
| cloud/auth | login | FAIL |
| cloud/auth | logout | PASS |
| cloud/auth | whoami | PASS |
| cloud | deploy | FAIL |
| cloud | dockerfile | FAIL |
| cloud | init | PASS |
| cloud/profile | create | FAIL |
| cloud/profile | delete | FAIL |
| cloud/profile | edit | FAIL |
| cloud/profile | list | PASS |
| cloud/profile | show | FAIL |
| cloud | restart | FAIL |
| cloud/secrets | list | FAIL |
| cloud/secrets | sync | FAIL |
| cloud | status | PASS |
| cloud/sync | local-content | PASS |
| cloud/sync | local-skills | PASS |
| cloud/sync | pull | FAIL |
| cloud/sync | push | PASS |
| cloud/tenant | create | FAIL |
| cloud/tenant | delete | FAIL |
| cloud/tenant | list | PASS |
| cloud/tenant | rotate-sync-token | FAIL |
| cloud/tenant | show | FAIL |
| logs/stream | cleanup | PASS |
| logs/stream | delete | PASS |
| logs/stream | view | FAIL |
| logs/trace | ai | FAIL |
| logs/trace | list | FAIL |
| logs/trace | lookup | FAIL |
| logs/trace | view | PASS |
| mcp | list-packages | PASS |
| mcp | list | PASS |
| mcp | validate | FAIL |
| services/db | assign-admin | FAIL |
| services/db | migrate | PASS |
| services/db | query | PASS |
| services/db | reset | FAIL |
| services/db | status | FAIL |
| services | restart | PASS |
| services/scheduler | list | PASS |
| services/scheduler | log-cleanup | FAIL |
| services/scheduler | run | FAIL |
| services/scheduler | session-cleanup | FAIL |
| services | start | PASS |
| services | status | PASS |
| services | stop | PASS |
| setup | setup | FAIL |

## Summary
- **Total Tests:** 56
- **Passed:** 26
- **Failed:** 30
- **Pass Rate:** 46.4%
