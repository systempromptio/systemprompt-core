//! Integration tests for `systemprompt-runtime`.
//!
//! Exercises `AppContext` context loaders, `StartupValidator::validate`,
//! and the validation-report display helpers end-to-end. These tests
//! deliberately stay clear of the shared fixture bootstrap — they construct
//! fresh `Config` values per test so the validator's early-bail
//! (services-config) path runs.

#![allow(clippy::all)]

#[cfg(test)]
mod app_context_traits;
#[cfg(test)]
mod bootstrap_fixture;
#[cfg(test)]
mod config_loaders;
#[cfg(test)]
mod display;
#[cfg(test)]
mod validate_database_path;
#[cfg(test)]
mod validator;
#[cfg(test)]
mod validator_bootstrapped;

#[cfg(test)]
pub(crate) async fn seed_config_admin(config: &systemprompt_manifest::Config) {
    let pool: systemprompt_database::DbPool =
        std::sync::Arc::new(systemprompt_database::Database::from_pools(
            std::sync::Arc::new(
                sqlx::PgPool::connect(&config.database_url)
                    .await
                    .expect("connect to the configured database"),
            ),
            None,
        ));
    let email = config
        .system_admin_email
        .as_ref()
        .expect("the fixture config names the admin email");
    systemprompt_test_fixtures::seed_user_row_with_roles(
        &pool,
        &systemprompt_identifiers::UserId::new(config.system_admin_username.clone()),
        email.as_str(),
        &["admin".to_owned(), "user".to_owned()],
    )
    .await
    .expect("seed the configured system admin");
}

#[cfg(test)]
pub(crate) async fn admin_app(database_url: &str) -> std::sync::Arc<systemprompt_runtime::AppContext> {
    seed_config_admin(&systemprompt_test_fixtures::fixture_config(database_url)).await;
    systemprompt_test_fixtures::test_app_context(database_url).await
}
