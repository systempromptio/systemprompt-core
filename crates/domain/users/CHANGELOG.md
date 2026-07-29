# Changelog

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `users.email` is stored normalised (trimmed, lowercased) with a schema `CHECK`; migration `009_normalise_user_emails` rewrites existing rows and fails loudly on rows differing only by case, naming `admin users merge` as the remedy. `normalise_email` is exported and applied at every create and lookup.
- **Breaking:** `merge_users` runs as one transaction rekeying every user-data table (sessions, tasks, messages, contexts, MCP executions/artifacts/sessions, governance decisions, logs, AI requests, engagement/analytics events, outbox, files, link clicks, fingerprint associations) instead of two tables non-atomically. Security artifacts deliberately die with the source row via FK cascade. `MergeResult` gains `total_rows`.
- **Breaking:** `find_or_create_federated` links a federated sign-in whose IdP asserts a verified email to the existing account with that normalised email, instead of minting a duplicate user; unverified emails keep the synthetic `.federated.local` separation.
- **Breaking:** `cleanup_anonymous_users` honours `enforce` — when `false` it reports the would-delete count — and reads its window from the `retention_days` parameter (default 30).

### Changed

- `merge_users` deletes the source's quota buckets by the new `(subject_kind = 'user', subject_id)` key, matching the subject-keyed `ai_quota_buckets` schema.

### Added

- `UserService::promote_anonymous` moves an anonymous visitor's history onto their registered account, refusing non-anonymous sources and self-merges. `UserRepository::count_old_anonymous` backs observe-mode reporting.

## [0.26.0] - 2026-07-28

### Added

- `UserRepository::create_if_absent` and `UserService::create_if_absent` insert a user and return `None` when the name or email is already taken, rather than surfacing a unique violation. The auto-provisioning paths resolve the *same* well-known identity from several processes at once, so each one read "absent" and then raced to insert; every loser got back a driver error it could only classify by matching on the message text.

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** the session analytics views are consolidated. `v_clean_traffic` and `v_engaged_traffic` are the canonical human-traffic predicate, `v_bot_sessions` carries the user-agent bot taxonomy, and the redundant `v_clean_human_traffic` plus the unused bot, referrer, and session analytics views are dropped (migration `008_canonical_traffic_views`). Migrate any external query that selected from a dropped view to the canonical three.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.

### Removed

- The `UserProviderImpl` wrapper is removed; `UserService` implements the user-provider trait directly.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Breaking

- `AuthenticatedUser`: `department`, `with_department`, and `department()` removed and replaced by `attributes: BTreeMap<String, serde_json::Value>` with `with_attributes` and `attributes()`.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Users repository surface unchanged.

## [0.10.0] - 2026-05-15

### Changed
- Migration SQL moved into `schema/migrations/NNN_*.sql` files, discovered by the
  crate `build.rs` and surfaced through `Extension::migrations()`.

## [0.9.2] - 2026-05-14

### Added
- API key issuance, hashing, and verification via `ApiKeyService`.
- Device certificate enrollment and rotation via `DeviceCertService`.
- `CleanupAnonymousUsersJob` re-export for scheduler registration.

### Changed
- Re-exported `UserProvider` and `RoleProvider` traits from `systemprompt-traits`.
- Migrated public errors to `thiserror`-derived `UserError` with `Result<T>` alias.

## [0.3.0] - 2026-04-22

### Changed
- Formatting cleanup in the `device_cert` repository.

## [0.1.21] - 2026-04-02

### Added
- `UserRepository::session_exists()` to check whether a session is active.
- `UserService::session_exists()` service method.
- Re-exported `CleanupAnonymousUsersJob` from the `jobs` module.

### Changed
- Exposed the `jobs` module for external registration.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to the Rust 2024 edition.

## [0.1.2] - 2026-02-03

### Changed
- Regenerated the SQLx offline query cache.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; aligned to the workspace 0.1.0 version line.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Changed
- Marked the users extension as required via `is_required() -> true`.

### Fixed
- Fixed schema validation for `VIEW`-based schemas.
- Added migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Adopted the distributed schema registration pattern; each domain crate now owns its SQL schemas via the `Extension` trait.
- Removed centralised module loaders from `systemprompt-loader`.

### Fixed
- Fixed `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
