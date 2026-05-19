use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use reqwest::Client;
use tokio::task::JoinSet;
use tokio::time;

use crate::config::LoadConfig;
use crate::metrics::Metrics;

pub async fn run_scenario_distributed<F, Fut>(
    config: &LoadConfig,
    nodes: &[String],
    per_node: &[Arc<Metrics>],
    scenario_fn: F,
) where
    F: Fn(Client, String, Option<String>, Arc<Metrics>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send,
{
    let scenario_fn = Arc::new(scenario_fn);
    let active_users = Arc::new(AtomicUsize::new(0));
    let shutdown = Arc::new(AtomicBool::new(false));
    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("    failed to build HTTP client: {e}");
            return;
        },
    };

    let mut user_handles = JoinSet::new();

    for stage in &config.stages {
        let target = stage.target_users;
        let current = active_users.load(Ordering::Relaxed);
        let stage_start = time::Instant::now();

        if target > current {
            spawn_users(
                current..target,
                &client,
                config,
                nodes,
                per_node,
                &shutdown,
                &active_users,
                &scenario_fn,
                &mut user_handles,
            );
        } else if target < current {
            shutdown.store(true, Ordering::Relaxed);
            time::sleep(std::time::Duration::from_millis(100)).await;
            shutdown.store(false, Ordering::Relaxed);

            let remaining = active_users.load(Ordering::Relaxed);
            spawn_users(
                target..remaining.min(current),
                &client,
                config,
                nodes,
                per_node,
                &shutdown,
                &active_users,
                &scenario_fn,
                &mut user_handles,
            );
        }

        let elapsed = stage_start.elapsed();
        if elapsed < stage.duration {
            time::sleep(stage.duration - elapsed).await;
        }

        println!(
            "    stage complete: {} concurrent users across {} nodes for {}s",
            target,
            nodes.len(),
            stage.duration.as_secs()
        );
    }

    shutdown.store(true, Ordering::Relaxed);

    while let Some(result) = user_handles.join_next().await {
        if let Err(e) = result {
            eprintln!("    user task error: {e}");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_users<F, Fut>(
    range: std::ops::Range<usize>,
    client: &Client,
    config: &LoadConfig,
    nodes: &[String],
    per_node: &[Arc<Metrics>],
    shutdown: &Arc<AtomicBool>,
    active_users: &Arc<AtomicUsize>,
    scenario_fn: &Arc<F>,
    handles: &mut JoinSet<()>,
) where
    F: Fn(Client, String, Option<String>, Arc<Metrics>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send,
{
    for idx in range {
        // Round-robin each virtual user onto a node so the load fans out evenly
        // even when stages add users in non-contiguous ranges.
        let node = idx % nodes.len();
        let client = client.clone();
        let base_url = nodes[node].clone();
        let token = config.token.clone();
        let metrics = Arc::clone(&per_node[node]);
        let shutdown = Arc::clone(shutdown);
        let active_users = Arc::clone(active_users);
        let scenario_fn = Arc::clone(scenario_fn);

        active_users.fetch_add(1, Ordering::Relaxed);

        handles.spawn(async move {
            while !shutdown.load(Ordering::Relaxed) {
                scenario_fn(
                    client.clone(),
                    base_url.clone(),
                    token.clone(),
                    Arc::clone(&metrics),
                )
                .await;
            }
            active_users.fetch_sub(1, Ordering::Relaxed);
        });
    }
}
