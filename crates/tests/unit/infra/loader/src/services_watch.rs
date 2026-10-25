//! A `ConfigWatch<ServicesConfig>` driven by the loader, as the server's
//! services watch is: an advance of the `services` epoch re-reads the tree,
//! and a tree that no longer loads keeps the last good config served.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_traits::{
    BackgroundTasks, CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, RepositoryError,
    WatchSignals,
};
use tempfile::TempDir;

const ROOT_V1: &str = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

const ROOT_V2: &str = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4100, 4999]
  mcp_port_range: [5000, 5999]
"#;

const ROOT_BROKEN: &str = "agents: [unterminated\n";

struct FixedVersion;

#[async_trait]
impl ConfigVersionSource for FixedVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        Ok(0)
    }
}

struct Harness {
    _tmp: TempDir,
    root: PathBuf,
    watch: ConfigWatch<ServicesConfig>,
    epochs: CacheEpochs,
    _tasks: BackgroundTasks,
}

fn write_root(root: &Path, body: &str) {
    std::fs::write(root, body).expect("write config.yaml");
}

fn start() -> Harness {
    let tmp = TempDir::new().expect("tempdir");
    let services = tmp.path().join("services");
    std::fs::create_dir_all(services.join("config")).expect("config dir");
    let root = services.join("config/config.yaml");
    write_root(&root, ROOT_V1);

    let watch = ConfigWatch::new(ConfigLoader::load_from_path(&root, None).expect("v1 loads"));
    let epochs = CacheEpochs::new();
    let tasks = BackgroundTasks::new();
    let signals = WatchSignals {
        domain: CacheDomain::Services,
        epochs: epochs.clone(),
        version: Arc::new(FixedVersion),
        poll: Duration::from_secs(3600),
    };
    let reload_root = root.clone();
    watch.spawn(&tasks, signals, move || {
        let root = reload_root.clone();
        async move { ConfigLoader::load_from_path(&root, None) }
    });
    Harness {
        _tmp: tmp,
        root,
        watch,
        epochs,
        _tasks: tasks,
    }
}

fn agent_port_floor(watch: &ConfigWatch<ServicesConfig>) -> u16 {
    watch.get().settings.agent_port_range.0
}

async fn advance_until(h: &Harness, done: impl Fn() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        h.epochs.advance(CacheDomain::Services);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_services_epoch_advance_serves_the_rewritten_tree() {
    let h = start();
    assert_eq!(agent_port_floor(&h.watch), 4000);

    write_root(&h.root, ROOT_V2);
    advance_until(&h, || agent_port_floor(&h.watch) == 4100).await;

    assert_eq!(agent_port_floor(&h.watch), 4100);
    assert!(h.watch.reloads() > 0);
}

#[tokio::test]
async fn a_tree_that_fails_to_load_keeps_the_last_good_config() {
    let h = start();
    write_root(&h.root, ROOT_BROKEN);
    assert!(
        ConfigLoader::load_from_path(&h.root, None).is_err(),
        "the broken tree must not load"
    );

    for _ in 0..20 {
        h.epochs.advance(CacheDomain::Services);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert_eq!(h.watch.reloads(), 0, "a failed load never swaps the value");
    assert_eq!(agent_port_floor(&h.watch), 4000);

    write_root(&h.root, ROOT_V2);
    advance_until(&h, || agent_port_floor(&h.watch) == 4100).await;
    assert_eq!(
        agent_port_floor(&h.watch),
        4100,
        "the next good tree is picked up"
    );
}

#[test]
fn the_services_domain_is_no_database_plane() {
    assert_eq!(CacheDomain::Services.config_plane(), None);
    assert_eq!(CacheDomain::parse("services"), Some(CacheDomain::Services));
    assert_eq!(CacheDomain::from_notification("config:services"), None);
}
