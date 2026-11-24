use std::sync::Arc;

use systemprompt_governance::authz::{AllowAllHook, NullAuditSink, SharedAuthzHook};

use crate::app_context::{
    fixture_admitted, fixture_app_context_with_hook, fixture_config, fixture_context_builder,
    offline_kernel, test_app_context,
};
use crate::bootstrap::ensure_test_bootstrap;
use crate::db::test_database_url;

fn allow_all_hook() -> SharedAuthzHook {
    Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)))
}

#[tokio::test]
async fn default_context_builds_with_the_fixture_admin() {
    let ctx = test_app_context(&test_database_url()).await;
    assert_eq!(
        ctx.config().system_admin_username,
        crate::credential::FIXTURE_SYSTEM_ADMIN_USERNAME
    );
}

#[tokio::test]
async fn each_owner_pair_is_minted_for_its_caller() {
    let pool = crate::db::test_db_pool().await;
    let first = crate::credential::seed_fixture_owners(&pool, "self-check")
        .await
        .expect("seed the first owners");
    let second = crate::credential::seed_fixture_owners(&pool, "self-check")
        .await
        .expect("seed the second owners");
    let ids = [
        &first.admin.user_id,
        &first.user.user_id,
        &second.admin.user_id,
        &second.user.user_id,
    ];
    let emails = [
        &first.admin.email,
        &first.user.email,
        &second.admin.email,
        &second.user.email,
    ];
    for (i, id) in ids.iter().enumerate() {
        assert!(ids[i + 1..].iter().all(|other| other != id), "owner ids are distinct");
        assert!(
            emails[i + 1..].iter().all(|other| *other != emails[i]),
            "owner emails are distinct"
        );
    }
    assert_ne!(
        first.admin.email,
        crate::credential::FIXTURE_SYSTEM_ADMIN_EMAIL,
        "a minted admin is not the shared system admin"
    );
}

#[tokio::test]
async fn hook_context_builds_on_the_extension_mode_profile() {
    fixture_app_context_with_hook(&test_database_url(), allow_all_hook())
        .await
        .expect("a supplied hook builds on the extension-mode profile");
}

#[tokio::test]
async fn hook_on_the_unrestricted_profile_is_refused() {
    let url = test_database_url();
    crate::credential::seed_fixture_system_admin(&crate::db::test_db_pool().await)
        .await
        .expect("seed the fixture admin");
    let builder = fixture_context_builder(
        ensure_test_bootstrap(),
        fixture_config(&url),
        fixture_admitted(),
    )
    .expect("fixture builder")
    .with_shared_authz_hook(allow_all_hook());
    let err = builder
        .build()
        .await
        .expect_err("an extension hook on an unrestricted profile is refused");
    assert!(
        format!("{err:#}").contains("unrestricted"),
        "refusal names the contradicting mode: {err:#}"
    );
}

#[test]
fn offline_kernel_builds_without_a_runtime() {
    assert!(tokio::runtime::Handle::try_current().is_err());
    drop(offline_kernel(Default::default()));
}
