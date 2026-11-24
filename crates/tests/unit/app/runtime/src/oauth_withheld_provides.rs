//! On a host that seeds no token authority the oauth member withholds its
//! session issuer and plugin-token minter (`ProvideError::Withheld`): both
//! types are reserved and resolve to nothing, so a reader learns the seam is
//! absent instead of holding a value that refuses every call.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::{
    FixtureSeedParts, fixture_admitted_of, fixture_core_members, fixture_kernel_seed, lazy_pg_pool,
    resolved_kernel,
};
use systemprompt_traits::{BackgroundTasks, DynPluginTokenMinter, DynSessionIssuer, FileStorage};

#[test]
fn without_a_token_authority_the_issuer_and_minter_are_withheld() {
    let members = fixture_core_members()
        .into_iter()
        .filter(|member| matches!(member.metadata().id, "users" | "oauth"))
        .collect();
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
    assert!(seed.find::<Arc<dyn TokenAuthority>>().is_none());
    let kernel = resolved_kernel(&pool, &fixture_admitted_of(members), seed)
        .unwrap_or_else(|e| panic!("users and oauth resolve over the host seeds: {e:#}"));
    assert!(kernel.state().find::<DynSessionIssuer>().is_none());
    assert!(kernel.state().find::<DynPluginTokenMinter>().is_none());
}
