# Changelog

## [0.11.0] - 2026-05-20

### Breaking
- The sync client no longer sends a static `SYNC_TOKEN` header. Operators must provision the `sys_sync` OAuth client (`provision_sync_oauth_client` in `systemprompt-oauth`) and configure the sync agent to mint a `client_credentials` Service-JWT.

### Added
- Service-JWT acquisition and rotation in the API client: tokens are requested via `client_credentials`, cached until expiry, and re-minted on a 401.
- Typed `source_id` on content-diff structures, replacing borrowed `&str`.

### Changed
- API client request signing, error mapping, and front-matter `format!` call sites cleaned up of redundant references.

## [0.9.2] - 2026-05-14

### Changed
- Document `SyncService::sync_all` partial-state reporting via `SyncOpState` (`NotStarted`, `Partial`, `Completed`, `Failed`).

### Removed
- Drop `SkillsDiffCalculator`, `SkillsLocalSync`, and skill export helpers; skills are now disk-only and no longer routed through this crate.
- Drop user upsert helpers from database sync; database round-trip now covers contexts only.

## [0.1.8] - 2026-03-20

### Fixed
- Fix test compilation for `SyncApiClient::new` returning `SyncResult<Self>`.

## [0.1.7] - 2026-02-19

### Added
- Add `AgentsDiffCalculator` for SHA-256 hash comparison of disk vs database agents.
- Add `AgentsLocalSync` for bidirectional disk-to-database agent sync.
- Add `export_agent_to_disk`, `generate_agent_config`, and `generate_agent_system_prompt` export helpers.
- Add `AgentDiffItem`, `AgentsDiffResult`, and `DiskAgent` sync model types.

## [0.1.6] - 2026-02-18

### Changed
- Replace local `SkillConfig` and `strip_frontmatter` with shared `DiskSkillConfig` from `systemprompt_models`.
- Replace magic string literals with the `SKILL_CONFIG_FILENAME` constant.
- Add `plugins` and `hooks` directories to `INCLUDE_DIRS` for cloud sync.

## [0.1.5] - 2026-02-17

### Changed
- Replace raw `String` IDs with typed `SkillId` and `SourceId` from `systemprompt_identifiers`.
- Replace `direction: String` with the `LocalSyncDirection` enum on `LocalSyncResult`.
- Use `SkillId` as `HashMap` key in `SkillsDiffCalculator`.

### Removed
- Remove playbook support: `PlaybooksDiffCalculator`, `PlaybooksLocalSync`, playbook diff models, and export helpers.

## [0.1.4] - 2026-02-11

### Changed
- Add connect timeout (10s) and request timeout (60s) to the sync HTTP client.

## [0.1.3] - 2026-02-07

### Fixed
- Forward `Authorization: Bearer` header through cloud proxy sync so tenant VMs no longer reject requests with 401.
- Align upload response field name (`files_uploaded`) between tenant API and cloud proxy.
- Return parsed upload server response instead of discarding it.
- Raise retry budget to 5 attempts with 2s initial delay for transient upstream 502 errors.

### Removed
- Remove unused `handle_empty_response` method from `SyncApiClient`.

## [0.1.2] - 2026-02-03

### Changed
- Regenerate SQLx offline query cache.

## [0.1.1] - 2026-02-03

### Changed
- Support nested playbook directory structures in diff calculator and sync.
- Use recursive `WalkDir` scanning for playbook discovery.
- Export playbooks to nested directories based on domain path separators.
- Clean up empty parent directories when deleting orphan playbooks.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; align with workspace `0.1.0`.

## [0.0.13] - 2026-01-26

### Added
- Add `PlaybooksDiffCalculator` for comparing disk and database playbooks.
- Add `PlaybooksLocalSync` for bidirectional disk-database playbook sync.
- Add `export_playbook_to_disk` and `generate_playbook_markdown` helpers.
- Add `DiskPlaybook`, `PlaybookDiffItem`, and `PlaybooksDiffResult` models.
- Support `services/playbook/{category}/{domain}.md` directory layout.

## [0.0.11] - 2026-01-26

### Changed
- Update content sync to use the simplified ingestion API without content-type filtering.

## [0.0.3] - 2026-01-22

### Fixed
- Fix schema validation for VIEW-based schemas.
- Add migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration: each domain crate owns its SQL schemas via the `Extension` trait.
- Remove centralised module loaders from `systemprompt-loader`.

### Fixed
- Fix `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
