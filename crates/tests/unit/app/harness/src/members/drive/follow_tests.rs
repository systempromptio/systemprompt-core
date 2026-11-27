use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use systemprompt_device::client::SpClient;
use systemprompt_drive::account_root;
use systemprompt_harness::extension::{ManifestApplier, SpContext, SpExtension};
use systemprompt_harness::members::drive::{DriveExtension, DriveSettings};
use systemprompt_traits::{CancellationToken, OwnedTask};

use crate::members::drive::support::{
    FakeInstance, FakeResponse, applied, context, eventually, http,
};

struct Running {
    extension: DriveExtension,
    applier: Arc<dyn ManifestApplier>,
    cancel: CancellationToken,
    task: OwnedTask<()>,
}

fn start(ctx: &SpContext) -> Running {
    let extension = DriveExtension::default();
    let applier = extension.appliers(ctx).remove(0);
    let service = extension.services(ctx).remove(0);
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let task = OwnedTask::spawn("drive-follow-test", async move { service.run(token).await });
    Running {
        extension,
        applier,
        cancel,
        task,
    }
}

fn expected_root(base: &Path, url: &str) -> PathBuf {
    account_root(base, SpClient::new(url, http()).unwrap().base_url())
}

fn pick(ctx: &SpContext, folder: &Path) {
    ctx.settings_for(&DriveExtension::ID)
        .set(&DriveSettings {
            folder: Some(folder.to_path_buf()),
        })
        .unwrap();
}

#[tokio::test]
async fn the_sync_follows_the_applied_account_into_the_picked_folder() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let picked = dir.path().join("picked");
    pick(&ctx, &picked);
    let running = start(&ctx);
    let sync = Arc::clone(running.extension.handle().sync());
    running
        .applier
        .apply(&applied(&fake.url, "account-1"))
        .unwrap();
    let want = expected_root(&picked, &fake.url);
    eventually("the sync starts in the picked folder", || {
        sync.root().as_deref() == Some(want.as_path())
    })
    .await;

    let moved = dir.path().join("moved");
    pick(&ctx, &moved);
    let want = expected_root(&moved, &fake.url);
    eventually("the sync moves with the preference", || {
        sync.root().as_deref() == Some(want.as_path())
    })
    .await;

    running.applier.clear();
    eventually("the sync stops once the Drive is withheld", || {
        sync.root().is_none()
    })
    .await;

    running
        .applier
        .apply(&applied(&fake.url, "account-1"))
        .unwrap();
    eventually("the sync starts again", || sync.root().is_some()).await;
    running.cancel.cancel();
    running.task.join().await.unwrap();
    assert!(sync.root().is_none());
}

#[tokio::test]
async fn the_default_folder_is_used_without_a_preference() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let dir = tempfile::tempdir().unwrap();
    let documents = dir.path().join("Documents");
    let ctx = context(dir.path(), Some(documents.clone()));
    let running = start(&ctx);
    let sync = Arc::clone(running.extension.handle().sync());
    running
        .applier
        .apply(&applied(&fake.url, "account-1"))
        .unwrap();
    let base = DriveExtension::default_folder(&ctx).unwrap();
    assert!(base.starts_with(&documents));
    let want = expected_root(&base, &fake.url);
    eventually("the sync starts in the default folder", || {
        sync.root().as_deref() == Some(want.as_path())
    })
    .await;
    running.cancel.cancel();
    running.task.join().await.unwrap();
}

#[tokio::test]
async fn without_any_folder_or_with_an_unreadable_preference_the_sync_stays_off() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), None);
    let running = start(&ctx);
    let sync = Arc::clone(running.extension.handle().sync());
    running
        .applier
        .apply(&applied(&fake.url, "account-1"))
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(sync.root().is_none());

    pick(&ctx, &dir.path().join("picked"));
    eventually("the sync starts once a folder is picked", || {
        sync.root().is_some()
    })
    .await;

    let mut broken = serde_json::Map::new();
    broken.insert("folder".to_owned(), json!(7));
    ctx.settings.write(&DriveExtension::ID, broken);
    eventually("an unreadable preference stops the sync", || {
        sync.root().is_none()
    })
    .await;
    running.cancel.cancel();
    running.task.join().await.unwrap();
}

#[tokio::test]
async fn an_unapplied_drive_never_starts_the_sync() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context(dir.path(), Some(dir.path().join("Documents")));
    pick(&ctx, &dir.path().join("picked"));
    let running = start(&ctx);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(running.extension.handle().sync().root().is_none());
    assert!(running.extension.handle().is_withheld());
    running.cancel.cancel();
    running.task.join().await.unwrap();
}
