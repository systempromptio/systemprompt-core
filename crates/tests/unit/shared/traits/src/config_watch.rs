//! `ConfigWatch`: readers see the last good value, an epoch advance on the
//! watched domain reloads it, a moved `config_version` reloads it when the
//! notification was lost, and a failed reload keeps serving the old value.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_traits::{
    BackgroundTasks, CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, RepositoryError,
    WatchSignals,
};

const POLL: Duration = Duration::from_secs(30);

#[derive(Default)]
struct FakeVersion(AtomicI64);

#[async_trait]
impl ConfigVersionSource for FakeVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

struct Harness {
    watch: ConfigWatch<u64>,
    epochs: CacheEpochs,
    version: Arc<FakeVersion>,
    source: Arc<AtomicU64>,
    failing: Arc<AtomicBool>,
    tasks: BackgroundTasks,
}

fn start() -> Harness {
    let epochs = CacheEpochs::new();
    let version = Arc::new(FakeVersion::default());
    let source = Arc::new(AtomicU64::new(0));
    let failing = Arc::new(AtomicBool::new(false));
    let tasks = BackgroundTasks::new();
    let watch = ConfigWatch::new(0_u64);
    let signals = WatchSignals {
        domain: CacheDomain::GatewayRoutes,
        epochs: epochs.clone(),
        version: Arc::clone(&version) as Arc<dyn ConfigVersionSource>,
        poll: POLL,
    };
    let (reload_source, reload_failing) = (Arc::clone(&source), Arc::clone(&failing));
    watch.spawn(&tasks, signals, move || {
        let (source, failing) = (Arc::clone(&reload_source), Arc::clone(&reload_failing));
        async move {
            if failing.load(Ordering::SeqCst) {
                return Err("database unavailable");
            }
            Ok(source.load(Ordering::SeqCst))
        }
    });
    Harness {
        watch,
        epochs,
        version,
        source,
        failing,
        tasks,
    }
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn an_epoch_advance_on_the_watched_domain_reloads() {
    let h = start();
    settle().await;
    assert_eq!(*h.watch.get(), 0);

    h.source.store(7, Ordering::SeqCst);
    h.epochs.advance(CacheDomain::GatewayRoutes);
    settle().await;
    assert_eq!(*h.watch.get(), 7);
    assert_eq!(h.watch.reloads(), 1);

    h.source.store(9, Ordering::SeqCst);
    h.epochs.advance(CacheDomain::Groups);
    settle().await;
    assert_eq!(
        *h.watch.get(),
        7,
        "another domain's epoch is not this plane"
    );
    assert!(h.tasks.shutdown(Duration::from_secs(1)).await.is_drained());
}

#[tokio::test(start_paused = true)]
async fn a_moved_version_reloads_on_the_next_poll_without_a_notification() {
    let h = start();
    settle().await;

    tokio::time::advance(POLL).await;
    settle().await;
    assert_eq!(h.watch.reloads(), 0, "an unchanged version does not reload");

    h.source.store(3, Ordering::SeqCst);
    h.version.0.store(1, Ordering::SeqCst);
    tokio::time::advance(POLL / 2).await;
    settle().await;
    assert_eq!(
        *h.watch.get(),
        0,
        "nothing reloads before the poll interval"
    );

    tokio::time::advance(POLL / 2).await;
    settle().await;
    assert_eq!(*h.watch.get(), 3);
    assert_eq!(h.watch.reloads(), 1);

    tokio::time::advance(POLL).await;
    settle().await;
    assert_eq!(
        h.watch.reloads(),
        1,
        "the reloaded version is now the seen one"
    );
    assert!(h.tasks.shutdown(Duration::from_secs(1)).await.is_drained());
}

#[tokio::test(start_paused = true)]
async fn a_failed_reload_keeps_the_last_value_and_retries_on_the_next_poll() {
    let h = start();
    settle().await;

    h.failing.store(true, Ordering::SeqCst);
    h.source.store(5, Ordering::SeqCst);
    h.version.0.store(1, Ordering::SeqCst);
    h.epochs.advance(CacheDomain::GatewayRoutes);
    settle().await;
    assert_eq!(*h.watch.get(), 0);
    assert_eq!(h.watch.reloads(), 0);

    h.failing.store(false, Ordering::SeqCst);
    tokio::time::advance(POLL).await;
    settle().await;
    assert_eq!(
        *h.watch.get(),
        5,
        "the version still differs from the last good load"
    );
    assert!(h.tasks.shutdown(Duration::from_secs(1)).await.is_drained());
}

#[tokio::test]
async fn shutdown_stops_the_driver() {
    let h = start();
    let outcome = h.tasks.shutdown(Duration::from_secs(5)).await;
    assert_eq!(outcome, systemprompt_traits::DrainOutcome::Drained);
}
