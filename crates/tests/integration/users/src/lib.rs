//! Integration tests for the `systemprompt-users` crate.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod admin_service;

#[cfg(test)]
mod banned_ip_repository;

#[cfg(test)]
mod declared_users;

#[cfg(test)]
mod directory_search;

#[cfg(test)]
mod role_membership;

#[cfg(test)]
mod user_repository;

#[cfg(test)]
mod user_service;
#[cfg(test)]
mod user_service_anonymous;

#[cfg(test)]
pub(crate) fn purge_plan() -> systemprompt_extension::AdmittedPurge {
    let admitted = systemprompt_test_fixtures::schema_admitted().expect("the core members admit");
    systemprompt_extension::AdmittedPurge::of(admitted.built()).expect("the core purge plan admits")
}
