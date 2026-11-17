//! Boot refuses when the users member's session store or IP-ban store is
//! absent: without the session store the kernel refuses the composition and
//! names the store; with a stand-in member in the users member's place that
//! provides everything the users member provides except the ban list, the
//! kernel refuses with `KernelError::Missing`, naming analytics' unmet
//! requirement on the ban list.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::{Extension, NotificationAudienceResolver, Resolved};
use systemprompt_governance::authz::SharedRoleDirectory;
use systemprompt_governance::contract::RoleDirectorySlot;
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, fixture_admitted_of, fixture_core_members,
    fixture_app_context_with_extensions, fixture_config,
};
use systemprompt_runtime::KernelError;
use systemprompt_traits::core_seam::RoleAudience;
use systemprompt_traits::{CoreSeam, DynSessionStore};
use systemprompt_users::{
    SessionRepository, UserRepository, UserRoleAudienceResolver, UsersRoleDirectory,
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

async fn context_without_users(
    extra: Vec<Box<dyn Extension>>,
) -> anyhow::Result<Arc<systemprompt_runtime::AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    let mut members: Vec<Box<dyn Extension>> = Vec::new();
    let mut extra = Some(extra);
    for member in fixture_core_members() {
        if member.metadata().id == "users" {
            members.extend(extra.take().into_iter().flatten());
        } else {
            members.push(member);
        }
    }
    fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        fixture_admitted_of(members),
    )
    .await
}

#[tokio::test]
async fn without_the_users_session_store_the_composition_is_refused_naming_the_store()
-> anyhow::Result<()> {
    let err = context_without_users(Vec::new())
        .await
        .expect_err("no session store is provided");
    let chain = format!("{err:#}");
    assert!(chain.contains("SessionStore"), "{chain}");
    Ok(())
}

#[tokio::test]
async fn without_the_users_ban_list_the_composition_is_refused_naming_the_store()
-> anyhow::Result<()> {
    let err = context_without_users(vec![Box::new(users_without_bans())])
        .await
        .expect_err("no IP-ban store is provided");
    let missing = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<KernelError>())
        .unwrap_or_else(|| panic!("the kernel refuses the composition: {err:#}"));
    let KernelError::Missing { owner, ty } = missing else {
        panic!("a member's unmet requirement refuses the composition: {missing:?}");
    };
    assert_eq!(owner.as_str(), "analytics");
    assert!(ty.contains("IpBanStore"), "{ty}");
    Ok(())
}
