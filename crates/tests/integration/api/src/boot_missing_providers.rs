//! The kernel refuses a composition whose users member's session store or
//! IP-ban store is absent: without the session store it names the store;
//! with a stand-in member in the users member's place that provides
//! everything the users member provides except the ban list, it refuses with
//! `KernelError::Missing`, naming analytics' unmet requirement on the ban
//! list. The API host refuses either composition earlier, because no admin
//! can be bootstrapped without the users member.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::{Extension, NotificationAudienceResolver, Resolved};
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_governance::authz::SharedRoleDirectory;
use systemprompt_governance::contract::RoleDirectorySlot;
use systemprompt_runtime::{AdminBootstrapUnavailable, Kernel, KernelError};
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::{
    FixtureSeedParts, Probe, ensure_test_bootstrap, fixture_admitted_of,
    fixture_app_context_with_extensions, fixture_config, fixture_core_members, fixture_kernel_seed,
    lazy_pg_pool, resolved_kernel,
};
use systemprompt_traits::core_seam::RoleAudience;
use systemprompt_traits::{
    BackgroundTasks, CoreSeam, DynSessionStore, DynSessionTelemetry, DynUserProvider, FileStorage,
};
use systemprompt_users::{
    SessionRepository, UserRepository, UserRoleAudienceResolver, UserService, UsersRoleDirectory,
};

fn users_without_bans() -> Probe {
    Probe::new("cov-users-without-bans", |c| {
        c.provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let sessions: DynSessionStore = Arc::new(SessionRepository::new(&db));
            Ok(sessions)
        })
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let telemetry: DynSessionTelemetry = Arc::new(SessionRepository::new(&db));
            Ok(telemetry)
        })
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let users: DynUserProvider =
                Arc::new(UserService::new(Arc::new(UserRepository::new(&db))));
            Ok(users)
        })
        .provide(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let resolver: Arc<dyn NotificationAudienceResolver> = Arc::new(
                UserRoleAudienceResolver::new(Arc::new(UserRepository::new(&db))),
            );
            Ok(CoreSeam::<dyn NotificationAudienceResolver, RoleAudience>::new(resolver))
        });
        c.slot::<RoleDirectorySlot>().directory(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let directory: SharedRoleDirectory = UsersRoleDirectory::shared(db.write_pool());
            Ok(directory)
        });
    })
}

fn members_without_users(extra: Vec<Box<dyn Extension>>) -> Vec<Box<dyn Extension>> {
    let mut members: Vec<Box<dyn Extension>> = Vec::new();
    let mut extra = Some(extra);
    for member in fixture_core_members() {
        if member.metadata().id == "users" {
            members.extend(extra.take().into_iter().flatten());
        } else {
            members.push(member);
        }
    }
    members
}

fn kernel_without_users(extra: Vec<Box<dyn Extension>>) -> Result<Kernel, KernelError> {
    let pool: DbPool = Arc::new(systemprompt_database::Database::from_pools(
        lazy_pg_pool(),
        None,
    ));
    let storage = tempfile::tempdir().expect("storage dir");
    let file_storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let (documents, _binder) = DocumentsHandlesSlot::unbound();
    let seed = fixture_kernel_seed(
        &pool,
        FixtureSeedParts {
            file_storage,
            background_tasks: BackgroundTasks::new(),
            event_router: EventRouter::local_only(Arc::new(EventBroadcasters::new())),
            documents,
        },
    );
    resolved_kernel(
        &pool,
        &fixture_admitted_of(members_without_users(extra)),
        seed,
    )
}

#[tokio::test]
async fn without_the_users_member_the_api_host_is_refused_before_the_kernel() -> anyhow::Result<()>
{
    let bootstrap = ensure_test_bootstrap();
    let err = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        fixture_admitted_of(members_without_users(Vec::new())),
    )
    .await
    .expect_err("no admin can be bootstrapped");
    let chain = format!("{err:#}");
    assert!(
        chain.contains(&AdminBootstrapUnavailable::NotEnabled.to_string()),
        "{chain}"
    );
    Ok(())
}

#[tokio::test]
async fn without_the_users_session_store_the_composition_is_refused_naming_the_store()
-> anyhow::Result<()> {
    let err = kernel_without_users(Vec::new()).expect_err("no session store is provided");
    let chain = std::iter::successors(Some(&err as &dyn std::error::Error), |e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert!(chain.contains("SessionTelemetry"), "{chain}");
    Ok(())
}

#[tokio::test]
async fn without_the_users_ban_list_the_composition_is_refused_naming_the_store()
-> anyhow::Result<()> {
    let missing = kernel_without_users(vec![Box::new(users_without_bans())])
        .expect_err("no IP-ban store is provided");
    let KernelError::Missing { owner, ty } = &missing else {
        panic!("a member's unmet requirement refuses the composition: {missing:?}");
    };
    assert_eq!(owner.as_str(), "analytics");
    assert!(ty.contains("IpBanStore"), "{ty}");
    Ok(())
}
