mod auth;
mod config;
mod metrics;
mod reporters;
mod runner;
mod runner_distributed;
mod scenarios;

use std::sync::Arc;

use clap::Parser;

use config::{LoadConfig, NodeId, ScenarioId};
use metrics::{Metrics, Report};
use reporters::OutputFormat;

#[derive(Parser)]
#[command(
    name = "systemprompt-loadtest",
    about = "Rust-native HTTP load testing"
)]
struct Cli {
    #[arg(long, default_value = "all")]
    scenario: String,

    #[arg(long, default_value = "ci")]
    profile: String,

    #[arg(long, default_value = "http://localhost:8080")]
    base_url: String,

    #[arg(long)]
    token: Option<String>,

    #[arg(long, default_value = "../systemprompt-web")]
    web_dir: String,

    // Required for token self-acquisition on cloud-less (air-gapped) deployments.
    #[arg(long, env = "SYSTEMPROMPT_ADMIN_EMAIL")]
    admin_email: Option<String>,

    #[arg(long, default_value = "welcome")]
    agent_id: String,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,

    #[arg(long)]
    out_file: Option<String>,

    // Comma-separated replica base URLs; virtual users round-robin across them.
    #[arg(long, value_delimiter = ',')]
    nodes: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let token = match cli.token {
        Some(t) => {
            println!("  Using provided token.");
            Some(t)
        },
        None => {
            println!("  Acquiring token from CLI...");
            match auth::acquire_token(&cli.web_dir, cli.admin_email.as_deref()) {
                Ok(t) => {
                    println!("  Token acquired ({} chars).", t.len());
                    Some(t)
                },
                Err(e) => {
                    eprintln!("  WARNING: Could not acquire token: {e}");
                    eprintln!("  Authenticated scenarios will fail.");
                    None
                },
            }
        },
    };

    let config = match cli.profile.as_str() {
        "ci" => LoadConfig::ci(cli.base_url.clone(), token),
        "default" => LoadConfig::default_profile(cli.base_url.clone(), token),
        "airgap" => LoadConfig::airgap(cli.base_url.clone(), token),
        "scaled" => LoadConfig::scaled(cli.base_url.clone(), token),
        "soak" => LoadConfig::soak(cli.base_url.clone(), token),
        "spike" => LoadConfig::spike(cli.base_url.clone(), token),
        other => {
            eprintln!(
                "Unknown profile: {other}. Use 'ci', 'default', 'airgap', 'scaled', 'soak', or \
                 'spike'."
            );
            std::process::exit(1);
        },
    };

    let nodes: Vec<String> = if cli.nodes.is_empty() {
        Vec::new()
    } else {
        cli.nodes.iter().map(|n| n.trim().to_string()).collect()
    };

    println!();
    println!("systemprompt-loadtest");
    println!("  base_url:  {}", config.base_url);
    println!("  profile:   {}", cli.profile);
    println!("  scenario:  {}", cli.scenario);
    if !nodes.is_empty() {
        println!("  nodes:     {}", nodes.join(", "));
    }
    println!(
        "  auth:      {}",
        if config.token.is_some() { "yes" } else { "no" }
    );
    println!();

    let scenarios: Vec<&str> = match cli.scenario.as_str() {
        "all" => vec![
            "api-latency",
            "agent-registry",
            "context-lifecycle",
            "hook-track",
            "oauth-session",
            "task-read",
            "governance-only",
            "gateway-inference",
            "sse-stream",
        ],
        s => vec![s],
    };

    if cli.scenario == "send-message" {
        eprintln!("WARNING: send-message triggers AI inference and costs money.");
        eprintln!("  This scenario is excluded from 'all' and must be run explicitly.");
        eprintln!();
    }

    let mut report = Report::new();

    for scenario_name in scenarios {
        println!("  running: {scenario_name}");

        if nodes.is_empty() {
            let metrics = Arc::new(Metrics::new());
            if !dispatch_single(scenario_name, &config, &metrics, &cli.agent_id).await {
                std::process::exit(1);
            }
            report.add(ScenarioId::new(scenario_name), &metrics);
        } else {
            let per_node: Vec<Arc<Metrics>> =
                (0..nodes.len()).map(|_| Arc::new(Metrics::new())).collect();
            if !dispatch_distributed(scenario_name, &config, &nodes, &per_node, &cli.agent_id).await
            {
                std::process::exit(1);
            }
            let snapshots: Vec<(NodeId, metrics::MetricsSnapshot)> = per_node
                .iter()
                .enumerate()
                .map(|(i, m)| (NodeId(i), m.snapshot()))
                .collect();
            report.add_distributed(ScenarioId::new(scenario_name), &snapshots);
        }
    }

    let passed = match cli.output {
        OutputFormat::Json => {
            let out_file = cli
                .out_file
                .clone()
                .unwrap_or_else(|| "loadtest-report.json".to_string());
            match reporters::json::write(&report, &config.thresholds, &out_file) {
                Ok(p) => {
                    println!("  JSON report written to {out_file}");
                    p
                },
                Err(e) => {
                    eprintln!("  Failed to write JSON report: {e}");
                    std::process::exit(1);
                },
            }
        },
        OutputFormat::Text => reporters::text::print(&report, &config.thresholds),
    };

    if !passed {
        std::process::exit(1);
    }
}

async fn dispatch_single(
    scenario_name: &str,
    config: &LoadConfig,
    metrics: &Arc<Metrics>,
    agent_id: &str,
) -> bool {
    match scenario_name {
        "api-latency" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::api_latency::run(c, u, t, m)
            })
            .await;
        },
        "agent-registry" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::agent_registry::run(c, u, t, m)
            })
            .await;
        },
        "context-lifecycle" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::context_lifecycle::run(c, u, t, m)
            })
            .await;
        },
        "hook-track" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::hook_track::run(c, u, t, m)
            })
            .await;
        },
        "oauth-session" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::oauth_session::run(c, u, t, m)
            })
            .await;
        },
        "task-read" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::task_read::run(c, u, t, m)
            })
            .await;
        },
        "governance-only" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::governance_only::run(c, u, t, m)
            })
            .await;
        },
        "gateway-inference" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::gateway_inference::run(c, u, t, m)
            })
            .await;
        },
        "sse-stream" => {
            runner::run_scenario(config, Arc::clone(metrics), |c, u, t, m| {
                scenarios::sse_stream::run(c, u, t, m)
            })
            .await;
        },
        "send-message" => {
            let agent_id = agent_id.to_string();
            runner::run_scenario(config, Arc::clone(metrics), move |c, u, t, m| {
                let agent_id = agent_id.clone();
                async move {
                    scenarios::send_message::run(c, u, t, m, &agent_id).await;
                }
            })
            .await;
        },
        other => {
            report_unknown_scenario(other);
            return false;
        },
    }
    true
}

async fn dispatch_distributed(
    scenario_name: &str,
    config: &LoadConfig,
    nodes: &[String],
    per_node: &[Arc<Metrics>],
    agent_id: &str,
) -> bool {
    use runner_distributed::run_scenario_distributed;

    match scenario_name {
        "api-latency" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::api_latency::run(c, u, t, m)
            })
            .await;
        },
        "agent-registry" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::agent_registry::run(c, u, t, m)
            })
            .await;
        },
        "context-lifecycle" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::context_lifecycle::run(c, u, t, m)
            })
            .await;
        },
        "hook-track" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::hook_track::run(c, u, t, m)
            })
            .await;
        },
        "oauth-session" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::oauth_session::run(c, u, t, m)
            })
            .await;
        },
        "task-read" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::task_read::run(c, u, t, m)
            })
            .await;
        },
        "governance-only" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::governance_only::run(c, u, t, m)
            })
            .await;
        },
        "gateway-inference" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::gateway_inference::run(c, u, t, m)
            })
            .await;
        },
        "sse-stream" => {
            run_scenario_distributed(config, nodes, per_node, |c, u, t, m| {
                scenarios::sse_stream::run(c, u, t, m)
            })
            .await;
        },
        "send-message" => {
            let agent_id = agent_id.to_string();
            run_scenario_distributed(config, nodes, per_node, move |c, u, t, m| {
                let agent_id = agent_id.clone();
                async move {
                    scenarios::send_message::run(c, u, t, m, &agent_id).await;
                }
            })
            .await;
        },
        other => {
            report_unknown_scenario(other);
            return false;
        },
    }
    true
}

fn report_unknown_scenario(name: &str) {
    eprintln!("Unknown scenario: {name}");
    eprintln!(
        "Available: api-latency, agent-registry, context-lifecycle, hook-track, oauth-session, \
         task-read, governance-only, gateway-inference, sse-stream, send-message, all"
    );
}
