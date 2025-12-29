# systemprompt-models Status

## Checklist Compliance

### Boundary Rules
| Rule | Status | Notes |
|------|--------|-------|
| R1.1 | No `sqlx` dependency | FAIL | Has sqlx |
| R1.2 | No `tokio` runtime | FAIL | Uses tokio |
| R1.3 | No `reqwest` / HTTP | PASS | None |
| R1.4 | No `std::fs` | FAIL | 6 files use std::fs |
| R1.5 | No `systemprompt-*` imports | PASS | Only shared crates |
| R1.6 | No `async fn` | FAIL | Some async functions |
| R1.7 | No mutable statics | FAIL | OnceLock in profile.rs |
| R1.8 | No singletons | FAIL | Same as above |

### Code Quality
| Rule | Status | Notes |
|------|--------|-------|
| C1 | File ≤ 300 lines | FAIL | 7 files over limit |
| C2 | No `unsafe` | PASS | None |
| C3 | No `unwrap()`/`panic!()` | PASS | Only in tests |
| C4 | No inline comments | FAIL | Many doc comments |
| C5 | clippy passes | PASS | Clean |
| C6 | fmt passes | PASS | Clean |

## Action Items
1. Split large files
2. Remove doc comments
3. Move tests to core/tests/
4. Remove std::fs usage
5. Evaluate sqlx/tokio dependencies
