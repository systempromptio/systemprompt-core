//! Unit tests for database lifecycle module

mod bootstrap_lock_failures;
mod fk_deferral;
mod install_phase_failure;
mod installation;
mod installation_foreign_keys;
mod lock_wait;
mod migration_cost_audit;
mod migration_edges;
mod migration_refs_errors;
mod migration_run_failures;
mod migration_triggers;
mod migrations;
mod repair_concurrency;
mod repair_db;
mod retirements;
mod schema_currency_db;
mod settled_drift;
mod settled_install;
mod validation_db;
mod validation_failure_boundaries;
