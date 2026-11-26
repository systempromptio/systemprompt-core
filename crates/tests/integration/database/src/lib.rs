//! Integration tests for the systemprompt-database lifecycle layer.
//!
//! These tests exercise the boundary between declarative `schema/*.sql`
//! files and imperative `Extension::migrations()` — specifically the
//! install-order invariant: migrations first, schema second. The schema
//! linter forbids `ALTER`/`DROP`/etc. in schemas, which means any column
//! that the schema references must already exist when the schema runs.

#[cfg(test)]
mod composition;

#[cfg(test)]
mod composition_schemas;

#[cfg(test)]
mod schema_migration_order;

#[cfg(test)]
mod down_migration;

#[cfg(test)]
mod migration_plan_status;

#[cfg(test)]
mod fresh_install_stamp;

#[cfg(test)]
mod fresh_install_retirement;

#[cfg(test)]
mod migration_slot_reuse;

#[cfg(test)]
mod interrupted_install;

#[cfg(test)]
mod declarative_object_refs;

#[cfg(test)]
mod migration_cost_static;

#[cfg(test)]
mod migration_refs_static;

#[cfg(test)]
mod migration_supersedes;

#[cfg(test)]
mod advisory_lock;

#[cfg(test)]
mod cascade_delete;

#[cfg(test)]
mod batch_atomicity;

#[cfg(test)]
mod projection_queue;

#[cfg(test)]
mod config_change_context;
