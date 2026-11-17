//! End-to-end `AppContextBuilder::build` runs against a full tempdir profile
//! and the live test database. Each test bootstraps its own process-global
//! profile/secrets/config (nextest is process-per-test), seeds or omits the
//! configured system-admin row, and asserts either the assembled context or
//! the exact `RuntimeError` variant.

use systemprompt_extension::{ContributionError, Contributions, Extension, ExtensionMetadata};
use systemprompt_runtime::{AppContext, KernelError, RuntimeError};
use systemprompt_test_fixtures::{core_members, fixture_admitted, fixture_admitted_of, fixture_core_members};

use crate::boot::{BootOptions, boot};

fn unique_admin(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .subsec_nanos();
    format!("covrt_{tag}_{}_{nanos}", std::process::id())
}

async fn seed_admin(url: &str, name: &str, status: &str, roles: &[&str]) -> sqlx::PgPool {
    let pool = sqlx::PgPool::connect(url).await.expect("connect test db");
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    sqlx::query("INSERT INTO users (id, name, email, status, roles) VALUES ($1, $2, $3, $4, $5)")
        .bind(format!("usr_{name}"))
        .bind(name)
        .bind(format!("{name}@example.test"))
        .bind(status)
        .bind(&roles)
        .execute(&pool)
        .await
        .expect("insert admin fixture user");
    pool
}

async fn remove_admin(pool: &sqlx::PgPool, name: &str) {
    sqlx::query("DELETE FROM users WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await
        .expect("remove admin fixture user");
}

#[tokio::test]
async fn build_assembles_full_context_with_pool_and_write_url() {
    let admin = unique_admin("ok");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        pool_settings: true,
        write_url: true,
        ..BootOptions::default()
    });
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["admin", "user"]).await;

    let ctx = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .with_migrations(true)
        .build()
        .await
        .expect("full build must succeed with a seeded admin");

    assert_eq!(ctx.system_admin().username(), admin);
    assert_eq!(ctx.config().system_admin_username, admin);
    assert_eq!(ctx.server_address(), "127.0.0.1:8080");
    assert!(
        ctx.config().database_write_url.is_some(),
        "write url from secrets must be threaded into the config"
    );
    assert!(
        ctx.content_config().is_some(),
        "the fixture services/content/config.yaml must be loaded"
    );
    assert!(
        ctx.content_routing().is_some(),
        "a loaded content config provides routing"
    );
    assert!(ctx.geoip_reader().is_none());
    assert_eq!(ctx.admitted().built().members().count(), fixture_core_members().len());

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_fails_when_system_admin_is_missing() {
    let admin = unique_admin("miss");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });

    let err = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .build()
        .await
        .expect_err("build must fail without the admin row");
    match err {
        RuntimeError::SystemAdminNotFound { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminNotFound, got: {other:?}"),
    }
}

#[tokio::test]
async fn build_fails_when_system_admin_is_inactive() {
    let admin = unique_admin("inact");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });
    let pool = seed_admin(&fixture.database_url, &admin, "inactive", &["admin"]).await;

    let err = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .build()
        .await
        .expect_err("build must fail with an inactive admin");
    match err {
        RuntimeError::SystemAdminInactive { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminInactive, got: {other:?}"),
    }

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_fails_when_system_admin_lacks_admin_role() {
    let admin = unique_admin("role");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["user"]).await;

    let err = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .build()
        .await
        .expect_err("build must fail when the admin role is missing");
    match err {
        RuntimeError::SystemAdminMissingRole { username } => assert_eq!(username, admin),
        other => panic!("expected SystemAdminMissingRole, got: {other:?}"),
    }

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_rehomes_the_legacy_context_onto_the_current_system_admin() {
    let previous = unique_admin("prev");
    let admin = unique_admin("cur");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });
    let pool = seed_admin(
        &fixture.database_url,
        &previous,
        "active",
        &["admin", "user"],
    )
    .await;
    seed_admin(&fixture.database_url, &admin, "active", &["admin", "user"]).await;
    let legacy = systemprompt_identifiers::ContextId::legacy_context_row();
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, name, kind) VALUES ($1, $2, 'Legacy (pre-context)', 'legacy')
         ON CONFLICT (context_id) DO UPDATE SET user_id = EXCLUDED.user_id",
    )
    .bind(legacy.as_str())
    .bind(format!("usr_{previous}"))
    .execute(&pool)
    .await
    .expect("legacy context owned by the previous admin");

    let ctx = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .build()
        .await
        .expect("a legacy context owned by an earlier admin must not block boot");
    assert_eq!(ctx.system_admin().username(), admin);

    let owner: String =
        sqlx::query_scalar("SELECT user_id FROM user_contexts WHERE context_id = $1")
            .bind(legacy.as_str())
            .fetch_one(&pool)
            .await
            .expect("legacy context row");
    assert_eq!(
        owner,
        format!("usr_{admin}"),
        "the legacy context follows the system admin"
    );

    remove_admin(&pool, &admin).await;
    remove_admin(&pool, &previous).await;
}

#[tokio::test]
async fn a_database_url_override_replaces_the_profile_database_and_its_write_url() {
    let admin = unique_admin("override");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        write_url: true,
        ..BootOptions::default()
    });
    let other = systemprompt_test_fixtures::DisposableDb::with_schema("db_url_override").await;
    let pool = seed_admin(other.url(), &admin, "active", &["admin", "user"]).await;

    let ctx = AppContext::builder(fixture.boot.clone(), fixture_admitted())
        .with_database_url(other.url())
        .build()
        .await
        .expect("the build must run against the overriding database");

    assert_eq!(ctx.config().database_url, other.url());
    assert!(
        ctx.config().database_write_url.is_none(),
        "the profile's write url names the database being overridden"
    );
    assert_eq!(ctx.system_admin().username(), admin);

    pool.close().await;
    other.drop_now().await;
}

#[derive(Debug)]
struct AbsentCache;

#[derive(Debug, Clone)]
struct ProvidedCache(u32);

struct StateExtension {
    requires_absent: bool,
}

impl Extension for StateExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covrt-state",
            name: "covrt-state",
            version: "1.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.provide::<ProvidedCache, _>(|_| Ok(ProvidedCache(7)));
        if self.requires_absent {
            c.requires::<AbsentCache>();
        }
        Ok(())
    }
}

fn state_admitted(requires_absent: bool) -> std::sync::Arc<systemprompt_extension::Admitted> {
    let mut members = core_members();
    members.push(Box::new(StateExtension { requires_absent }));
    fixture_admitted_of(members)
}

#[tokio::test]
async fn build_fails_with_the_typed_unmet_state_requirement() {
    let admin = unique_admin("unmet");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["admin", "user"]).await;

    let err = AppContext::builder(fixture.boot.clone(), state_admitted(true))
        .build()
        .await
        .expect_err("a requirement nothing provides fails the build");
    match err {
        RuntimeError::Kernel(KernelError::Missing { owner, ty }) => {
            assert_eq!(owner.as_str(), "covrt-state");
            assert!(ty.ends_with("AbsentCache"), "got: {ty}");
        },
        other => panic!("expected KernelError::Missing, got: {other:?}"),
    }

    remove_admin(&pool, &admin).await;
}

#[tokio::test]
async fn build_seeds_the_kernel_entries_and_keeps_provided_state() {
    use systemprompt_extension::ExtensionContext;

    let admin = unique_admin("state");
    let fixture = boot(&BootOptions {
        admin_username: admin.clone(),
        ..BootOptions::default()
    });
    let pool = seed_admin(&fixture.database_url, &admin, "active", &["admin", "user"]).await;

    let ctx = AppContext::builder(fixture.boot.clone(), state_admitted(false))
        .build()
        .await
        .expect("the provided state satisfies the build");

    let state = ctx.extension_state();
    assert_eq!(state.get::<ProvidedCache>().map(|c| c.0), Ok(7));
    let pool_entry = state
        .get::<systemprompt_database::DbPool>()
        .expect("the kernel seeds the database pool");
    assert!(std::sync::Arc::ptr_eq(pool_entry, ctx.db_pool()));

    remove_admin(&pool, &admin).await;
}

#[derive(Debug)]
struct ProbeMarketplaceFilter;

#[async_trait::async_trait]
impl systemprompt_marketplace::MarketplaceFilter for ProbeMarketplaceFilter {
    async fn filter(
        &self,
        _user_id: &systemprompt_identifiers::UserId,
        candidate: systemprompt_marketplace::MarketplaceCandidate,
    ) -> Result<
        systemprompt_marketplace::MarketplaceCandidate,
        systemprompt_marketplace::MarketplaceFilterError,
    > {
        Ok(candidate)
    }
}

fn marketplace_filter_of(contribute_filter: bool) -> String {
    use std::sync::Arc;
    use systemprompt_extension::{FactoryError, Resolved, Slot, StandingTag};
    use systemprompt_governance::contract::RoleDirectorySlot;
    use systemprompt_marketplace::MarketplaceFilter;
    use systemprompt_marketplace::contract::{MarketplaceSlot, MarketplaceSlotBuilder};
    use systemprompt_test_fixtures::slots::built_slot;

    let roles = RoleDirectorySlot::build(
        Vec::new(),
        &systemprompt_test_fixtures::empty_resolved_kernel().resolved(),
    )
    .expect("an empty role directory slot builds");
    let kernel = systemprompt_test_fixtures::offline_resolved_kernel(
        &systemprompt_test_fixtures::probe_admitted(systemprompt_test_fixtures::ComposedParts::default()),
        systemprompt_extension::prelude::Dependencies::new().with(Arc::new(roles)),
    );
    let mut builder = MarketplaceSlotBuilder::default();
    if contribute_filter {
        builder.filter(
            |_: &Resolved<'_>| -> Result<Arc<dyn MarketplaceFilter>, FactoryError> {
                Ok(Arc::new(ProbeMarketplaceFilter))
            },
        );
    }
    let seams = built_slot::<MarketplaceSlot>(
        vec![("marketplace-probe", StandingTag::Core, builder)],
        &kernel.resolved(),
    )
    .expect("an opened marketplace slot is built");
    format!("{:?}", seams.filter())
}

#[test]
fn a_contributed_marketplace_filter_is_the_built_filter() {
    let filter = marketplace_filter_of(true);
    assert!(
        filter.contains("ProbeMarketplaceFilter"),
        "the admitted probe's filter must be built: {filter}"
    );
}

#[test]
fn with_no_contributed_filter_the_built_filter_allows_all() {
    let filter = marketplace_filter_of(false);
    assert!(
        filter.contains("AllowAllFilter") && !filter.contains("ProbeMarketplaceFilter"),
        "no contributed filter must build the default: {filter}"
    );
}
