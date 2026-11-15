//! Docker-backed `PostgreSQL` provisioning for local tenants.
//!
//! Each local tenant owns a compose project, so provisioning matches what the
//! template's local setup produces and two installations on one host never
//! contend for a container, a volume, a port, or a role. Public surface: the
//! container descriptor and the project lifecycle helpers consumed by the
//! create and delete flows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod container;

pub use container::TenantContainer;
pub(super) use container::{
    compose_path_for_project, generate_admin_password, is_project_running, nanoid,
    new_local_tenant_id, remove_project, start_project,
};
