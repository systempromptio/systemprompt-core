//! Local Docker-backed `PostgreSQL` provisioning for local tenants.
//!
//! Manages the shared `systemprompt-postgres` container and its volume, and
//! creates, drops, and authorises per-tenant databases inside it via
//! `docker exec psql`. Public surface: the shared-config types and the
//! container/database lifecycle helpers consumed by the create/delete flows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
pub mod container;
pub mod database;

pub use config::SharedContainerConfig;
pub(super) use config::{
    SHARED_ADMIN_USER, SHARED_PORT, SHARED_VOLUME_NAME, load_shared_config, save_shared_config,
};
pub(in crate::commands::cloud) use container::wait_for_postgres_healthy;
pub(super) use container::{
    check_volume_exists, generate_admin_password, generate_shared_postgres_compose,
    get_container_password, is_shared_container_running, nanoid, new_local_tenant_id,
    remove_shared_volume, stop_shared_container,
};
pub(super) use database::{
    create_database_for_tenant, drop_database_for_tenant, ensure_admin_role,
};
