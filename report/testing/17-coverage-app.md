# Application Layer Coverage

## Current State

The application layer contains four crates: scheduler, runtime, generator, and sync. Coverage ranges from good (scheduler at 85%) to moderate (runtime, generator, and sync at approximately 50% effective coverage). The sync crate has the largest test file in the codebase at 2,262 lines, well beyond the 300-line limit.

### Scheduler (crates/app/scheduler/) -- GOOD

- **Source**: ~7 files
- **Tests**: 12+ test files, 237 tests
- **Coverage**: ~85%
- **Quality**: Job scheduling, cron parsing, and task management are tested.
- **Gap**: Orchestration reconciler is untested. No concurrent job execution tests exist.

### Runtime (crates/app/runtime/) -- MODERATE

- **Source**: ~9 files
- **Tests**: 9 test files, 230 tests
- **Coverage**: ~50% effective
- **Quality**: AppContext construction is tested but lifecycle management is not.
- **Gap**: Service startup/shutdown sequencing and error recovery are untested.

### Generator (crates/app/generator/) -- MODERATE

- **Source**: ~9 files
- **Tests**: 9 test files, 101 tests
- **Coverage**: ~50% effective
- **Quality**: Static site generation basics are tested.
- **Gap**: Template rendering edge cases and asset pipeline behavior are untested.

### Sync (crates/app/sync/) -- MODERATE

- **Source**: ~5 files
- **Tests**: 5 test files, 281 tests
- **Coverage**: Numbers are high but quality varies.
- **Quality**: Skill sync direction and cloud sync are tested.
- **Gap**: Conflict resolution, retry logic, and partial failure handling are untested.
- **Issue**: Largest test file in the codebase at 2,262 lines, violating the 300-line limit.
- **Issue**: 5 tests were recently broken by a `skill_id` to `id` rename and have been excluded from workspace tests.

### Risk Assessment

The scheduler is well-protected but lacks concurrency testing, which is critical for a job scheduler. The runtime crate's untested lifecycle management means service startup failures or shutdown ordering issues could go undetected. The sync crate's broken tests and oversized test file indicate maintenance debt that will compound over time. The generator's untested template rendering could produce incorrect static output silently.

## Desired State

- Scheduler reaches 95% coverage with concurrent job execution tests and orchestration reconciler tests.
- Runtime reaches 75%+ coverage with lifecycle management tests covering startup sequencing, graceful shutdown, and error recovery.
- Generator reaches 75%+ coverage with template rendering edge case tests and asset pipeline tests.
- Sync crate's oversized test file is split into focused modules under 300 lines each.
- Sync crate's 5 broken tests are fixed and re-enabled in the workspace.
- Sync crate adds tests for conflict resolution, retry logic, and partial failure handling.
- All application layer crates maintain a minimum of 75% effective coverage.

## How to Get There

### Phase 1: Fix Sync Crate Issues (Highest Priority)

1. Fix the 5 broken tests caused by the `skill_id` to `id` rename and re-enable them in workspace tests.
2. Split the 2,262-line test file into focused modules, each under 300 lines, organized by functionality (direction tests, cloud sync tests, conflict tests, error tests).
3. Add tests for conflict resolution when local and remote changes diverge.
4. Add tests for retry logic when sync operations fail transiently.
5. Add tests for partial failure handling when some items sync and others fail.

### Phase 2: Runtime Lifecycle Tests

1. Write tests for service startup sequencing, verifying that dependencies start before dependents.
2. Write tests for graceful shutdown, verifying that services stop in reverse order and in-flight work completes.
3. Write tests for error recovery when a service fails during startup (rollback behavior, error reporting).
4. Write tests for AppContext lifecycle events (creation, ready, shutdown signals).

### Phase 3: Generator Edge Cases

1. Write tests for template rendering with edge cases: missing variables, nested includes, recursive templates, malformed input.
2. Write tests for asset pipeline behavior: CSS/JS bundling, image optimization, cache busting.
3. Write tests for incremental generation (only regenerate changed pages).
4. Write tests for error handling when template files are missing or malformed.

### Phase 4: Scheduler Concurrency

1. Write tests for concurrent job execution verifying that jobs run in parallel when resources allow.
2. Write tests for job queue behavior under contention (multiple workers, priority ordering).
3. Write tests for the orchestration reconciler verifying desired-state convergence.
4. Write tests for job failure and retry behavior under concurrent execution.

## Incremental Improvement Strategy

### Week 1-2: Sync Crate Stabilization

Target: Fix the 5 broken tests, split the oversized test file into 4-6 focused modules, and add 3 new test files for conflict resolution and retry logic. This addresses active maintenance debt and prevents it from growing. Expected result: sync crate has all tests passing, file sizes comply with the 300-line limit, and coverage quality improves.

### Week 3-4: Runtime Lifecycle

Target: 4 new test files covering service startup sequencing, graceful shutdown, error recovery, and AppContext lifecycle. These are the most impactful gaps in the runtime crate. Expected result: runtime effective coverage rises from 50% to approximately 70%.

### Week 5-6: Generator Template and Asset Tests

Target: 4 new test files covering template rendering edge cases, asset pipeline behavior, incremental generation, and error handling. Expected result: generator effective coverage rises from 50% to approximately 70%.

### Week 7-8: Scheduler Concurrency

Target: 3 new test files covering concurrent job execution, queue contention, and orchestration reconciliation. Expected result: scheduler coverage rises from 85% to approximately 92%.

### Ongoing

Enforce the 300-line test file limit to prevent future oversized files. Fix broken tests immediately rather than excluding them from the workspace. Add concurrency tests for any new job types or scheduling patterns. Target 80%+ effective coverage across all application layer crates by end of quarter.
