//! Owned-process lifecycle and setup-flow tests for the `systemprompt` binary,
//! linked as one test target. Each module drives its own isolated fixture or
//! disposable database.

mod cloud_doctor_recovery;
mod serve_lifecycle;
mod subprocess_bridge_list_lifecycle;
mod subprocess_profile_migration_wrapper;
mod subprocess_reporting_evidence;
mod subprocess_setup_migration_wrapper;
mod subprocess_setup_wizard;
