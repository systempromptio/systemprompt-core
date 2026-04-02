# Domain: Content, Files, Analytics, Users, Templates Coverage

## Content (crates/domain/content/)

### Current State

- **Source**: 37 files across 6 service modules
- **Tests**: 20 test files (302 tests)
- **Effective coverage**: ~30-35%
- **Tested**: Link validation, basic ingestion
- **Untested**: content_provider (main service), search, link tracking, content pipeline
- **Integration quality**: 30 integration tests exist but 22 have no assertions, making them hollow pass-through tests that verify nothing beyond "it compiles and doesn't panic"

### Desired State

- Effective coverage reaches 70%+ with meaningful behavioral assertions
- content_provider service has full unit test coverage for its public API
- Search functionality tested with various query patterns, edge cases, and empty results
- Link tracking tested end-to-end: creation, click recording, expiry
- Content pipeline tested for each stage: ingestion, transformation, storage, retrieval
- All 22 hollow integration tests either gain real assertions or are removed

### How to Get There

1. **Fix hollow integration tests first** (22 tests): Add assertions that verify return values, side effects, or state changes. If the test cannot meaningfully assert anything, delete it.
2. **Add content_provider service tests**: Mock the repository layer, test each public method for success paths, error paths, and edge cases (empty content, oversized content, duplicate content).
3. **Add search tests**: Test query parsing, result ranking, pagination, empty results, and special characters in queries.
4. **Add link tracking tests**: Test link creation, click recording with deduplication, link expiry, and invalid link handling.
5. **Add content pipeline tests**: Test each pipeline stage independently, then add integration tests that verify the full pipeline from ingestion to retrieval.

### Incremental Improvement Strategy

- **Week 1**: Fix or delete the 22 hollow integration tests. This immediately improves signal-to-noise ratio without adding new code.
- **Week 2**: Add content_provider service unit tests (success and error paths for each public method).
- **Week 3**: Add search and link tracking tests.
- **Week 4**: Add content pipeline stage tests and full pipeline integration tests.

---

## Files (crates/domain/files/)

### Current State

- **Source**: 6 service files (upload, validation, error handling)
- **Tests**: 15 test files (347 tests) -- number looks high but quality is critically low
- **Effective coverage**: 0% of service logic
- **Tested**: Model serialization, file category mappings
- **Untested**: Upload service, file validation, error handling
- **Integration quality**: 39 integration tests -- ALL have zero assertions (100% hollow)
- **Risk**: File upload bugs (path traversal, size validation) go completely undetected

### Desired State

- Upload service tested for valid uploads, oversized files, invalid MIME types, and path traversal attempts
- File validation tested for all supported and unsupported file types
- Error handling tested for every error variant with correct HTTP status codes
- All 39 hollow integration tests either gain real assertions or are removed
- Security-critical paths (path traversal, size limits) have dedicated test coverage

### How to Get There

1. **Delete or fix all 39 hollow integration tests**: These provide false confidence. Either add assertions or remove them entirely.
2. **Add upload service unit tests**: Mock the storage backend. Test successful upload, oversized file rejection, invalid MIME type rejection, path traversal prevention, and concurrent upload handling.
3. **Add file validation tests**: Test each validation rule independently -- file size limits, allowed extensions, MIME type checking, filename sanitization.
4. **Add error handling tests**: Verify each error variant maps to the correct user-facing error message and HTTP status code.
5. **Add security-focused tests**: Explicitly test path traversal payloads (e.g., `../../etc/passwd`), null bytes in filenames, and extremely long filenames.

### Incremental Improvement Strategy

- **Week 1**: Delete the 39 hollow integration tests. Add 5 upload service unit tests covering the critical path (valid upload, oversized, bad MIME, path traversal, missing file).
- **Week 2**: Add file validation tests for each rule and error handling tests for each variant.
- **Week 3**: Add security-focused tests and edge case tests (empty files, zero-byte files, files with unicode names).

---

## Analytics (crates/domain/analytics/)

### Current State

- **Source**: 65 files across service modules
- **Tests**: 17 test files (587 tests)
- **Effective coverage**: ~40%
- **Tested**: Anomaly detection, extraction logic, throttling (these are genuinely good tests)
- **Untested**: Behavioral detection, bot filtering, event persistence
- **Strength**: Analytics extractor tests have real behavioral assertions with user-agent testing -- best example of meaningful test quality in the domain layer

### Desired State

- Effective coverage reaches 65%+
- Behavioral detection tested with realistic bot patterns and legitimate user patterns
- Bot filtering tested against known bot user-agent strings, rate patterns, and behavioral signals
- Event persistence tested for batch writes, deduplication, and data integrity
- Existing high-quality extractor tests serve as the template for new test patterns

### How to Get There

1. **Use extractor tests as the pattern**: The existing extractor tests demonstrate how to write meaningful behavioral assertions. New tests should follow this pattern.
2. **Add behavioral detection tests**: Create test fixtures with known bot behavioral patterns (rapid sequential requests, no mouse movement, predictable timing) and legitimate user patterns. Assert correct classification.
3. **Add bot filtering tests**: Test against a curated list of known bot user-agents, rate-based detection thresholds, and behavioral signal combinations.
4. **Add event persistence tests**: Mock the database layer. Test batch insertion, deduplication logic, and data integrity constraints.

### Incremental Improvement Strategy

- **Week 1**: Add behavioral detection tests using the extractor test pattern as a template.
- **Week 2**: Add bot filtering tests with realistic user-agent fixtures.
- **Week 3**: Add event persistence tests with mocked database layer.

---

## Users (crates/domain/users/)

### Current State

- **Source**: 23 files
- **Tests**: 11 test files (253 tests)
- **Effective coverage**: ~48% (2 of 3 main service files tested)
- **Tested**: User model validation, basic CRUD
- **Untested**: Cleanup job execution (recently made public), admin operations
- **Integration quality**: 84 integration tests -- ALL have zero assertions (100% hollow)

### Desired State

- Effective coverage reaches 70%+
- All 3 main service files have unit test coverage
- Cleanup job execution tested for correct deletion logic, dry-run mode, and error handling
- Admin operations tested for permission checks and audit logging
- All 84 hollow integration tests either gain real assertions or are removed

### How to Get There

1. **Fix or delete 84 hollow integration tests**: This is the single highest-impact action. These tests consume CI time while providing zero signal.
2. **Add cleanup job tests**: Test that the cleanup job correctly identifies expired/orphaned users, respects dry-run mode, handles partial failures gracefully, and logs actions for audit.
3. **Add admin operation tests**: Test permission boundary enforcement (non-admin cannot perform admin operations), audit log creation, and bulk operation handling.
4. **Add the missing service file tests**: Identify the untested service file and add unit tests for its public API.

### Incremental Improvement Strategy

- **Week 1**: Delete or fix the 84 hollow integration tests. This is the highest-leverage action across all domain crates.
- **Week 2**: Add cleanup job execution tests and the missing service file tests.
- **Week 3**: Add admin operation tests with permission boundary verification.

---

## Templates (crates/domain/templates/)

### Current State

- **Source**: 6 files
- **Tests**: 6 test files (145 tests -- 98 sync, 47 async)
- **Effective coverage**: ~83% (1:1 file ratio)
- **Tested**: Registry operations, builder patterns, rendering pipeline
- **Quality**: Good behavioral tests with async rendering
- **Mock infrastructure**: MockComponent, MockPageProvider, MockTemplateLoader -- best mock examples in the codebase
- **Assessment**: This is one of the best-tested domain crates

### Desired State

- Coverage remains at 80%+ as the crate evolves
- Error path coverage added for rendering failures, missing templates, and invalid builder configurations
- Mock infrastructure documented as the recommended pattern for other crates to follow
- Performance-sensitive rendering paths have benchmark coverage

### How to Get There

1. **Add error path tests**: Test rendering with missing partials, circular template includes, and malformed template syntax.
2. **Add builder edge case tests**: Test builder with missing required fields, duplicate registrations, and conflicting configurations.
3. **Document mock patterns**: Extract MockComponent, MockPageProvider, and MockTemplateLoader patterns into guidance that other crate test authors can follow.
4. **Add async error tests**: Test async rendering with timeout scenarios and concurrent rendering conflicts.

### Incremental Improvement Strategy

- **Week 1**: Add error path tests for rendering failures and missing templates.
- **Week 2**: Add builder edge case tests and async error tests.
- **Ongoing**: Use this crate's mock infrastructure as the reference pattern when writing tests for other domain crates.
