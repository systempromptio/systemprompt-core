# systemprompt-core-scheduler Unit Tests

## Crate Overview
Background job scheduler and service management. Handles job scheduling, service reconciliation, and process lifecycle.

## Source Files
- `src/services/` - SchedulerService, ServiceManagementService, ServiceReconciler
- `src/repository/` - SchedulerRepository
- `src/jobs/` - Job definitions
- `src/models/` - ScheduledJob, JobStatus, ServiceConfig

## Test Plan

### SchedulerService Tests
- `test_scheduler_schedule_job` - Schedule job
- `test_scheduler_cancel_job` - Cancel job
- `test_scheduler_job_execution` - Execute job
- `test_scheduler_recurring_jobs` - Recurring jobs

### Service Management Tests
- `test_service_start` - Start service
- `test_service_stop` - Stop service
- `test_service_restart` - Restart service
- `test_service_status` - Get status

### Service Reconciler Tests
- `test_reconciler_detect_drift` - Detect drift
- `test_reconciler_correct_state` - Correct state

### Job Tests
- `test_database_cleanup_job` - Database cleanup
- `test_cleanup_empty_contexts_job` - Context cleanup
- `test_cleanup_inactive_sessions_job` - Session cleanup
- `test_behavioral_analysis_job` - Behavioral analysis
- `test_feature_extraction_job` - Feature extraction

### Repository Tests
- `test_job_persistence` - Persist job
- `test_job_query` - Query jobs
- `test_job_status_update` - Update status

## Mocking Requirements
- Mock database
- Mock clock
- Mock job executors

## Test Fixtures Needed
- Sample job configs
- Sample service configs
