# systemprompt-sync Unit Tests

## Crate Overview
Synchronization of files, database, and crate deployments. Handles diff calculation, file sync, and export functionality.

## Source Files
- `src/diff/` - ContentDiffCalculator, SkillsDiffCalculator
- `src/export/` - Export functions
- `src/files/` - FileSyncService
- `src/local/` - ContentLocalSync, SkillsLocalSync
- `src/database/` - DatabaseSyncService
- `src/crate_deploy/` - CrateDeployService
- `src/api_client/` - SyncApiClient

## Test Plan

### Diff Calculator Tests
- `test_content_diff_no_changes` - No changes
- `test_content_diff_additions` - Added content
- `test_content_diff_deletions` - Deleted content
- `test_content_diff_modifications` - Modified content
- `test_skills_diff_calculation` - Skills diff

### File Sync Tests
- `test_file_sync_push` - Push files
- `test_file_sync_pull` - Pull files
- `test_file_sync_conflict` - Conflict detection

### Export Tests
- `test_export_content_to_file` - Export content
- `test_export_skill_to_disk` - Export skill
- `test_generate_content_markdown` - Generate markdown
- `test_generate_skill_config` - Generate config

### Local Sync Tests
- `test_local_sync_content` - Sync content
- `test_local_sync_skills` - Sync skills

### Database Sync Tests
- `test_database_export` - Export database
- `test_database_import` - Import database

### Hash Computation Tests
- `test_compute_content_hash` - Content hash
- `test_hash_consistency` - Hash consistency

## Mocking Requirements
- Mock filesystem
- Mock database
- Mock API client

## Test Fixtures Needed
- Sample content files
- Sample skill files
