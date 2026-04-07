mod auth;
mod config;
mod metrics;
mod runner;
mod scenarios;

use std::sync::Arc;

use clap::Parser;

use config::LoadConfig;
use metrics::{Metrics, Report};

#[derive(Parser)]
#[command(name = "systemprompt-loadtest", about = "Rust-native HTTP load testing")]
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

    #[arg(long, default_value = "welcome")]
    agent_id: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let token = match cli.token {
        Some(t) => {
            println!("  Using provided token.");
            Some(t)
        }
        None => {
            println!("  Acquiring token from CLI...");
            match auth::acquire_token(&cli.web_dir) {
                Ok(t) => {
                    println!("  Token acquired ({} chars).", t.len());
                    Some(t)
                }
                Err(e) => {
                    eprintln!("  WARNING: Could not acquire token: {e}");
                    eprintln!("  Authenticated scenarios will fail.");
                    None
                }
            }
        }
    };

    let config = match cli.profile.as_str() {
        "ci" => LoadConfig::ci(cli.base_url.clone(), token),
        "default" => LoadConfig::default_profile(cli.base_url.clone(), token),
        other => {
            eprintln!("Unknown profile: {other}. Use 'ci' or 'default'.");
            std::process::exit(1);
        }
    };

    println!();
    println!("systemprompt-loadtest");
    println!("  base_url:  {}", config.base_url);
    println!("  profile:   {}", cli.profile);
    println!("  scenario:  {}", cli.scenario);
    println!("  auth:      {}", if config.token.is_some() { "yes" } else { "no" });
    println!();

    let scenarios: Vec<&str> = match cli.scenario.as_str() {
        "all" => vec![
            "api-latency",
            "agent-registry",
            "context-lifecycle",
            "hook-track",
            "oauth-session",
            "task-read",
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

        let metrics = Arc::new(Metrics::new());

        match scenario_name {
            "api-latency" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::api_latency::run(client, base_url, token, m)
                })
                .await;
            }
            "agent-registry" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::agent_registry::run(client, base_url, token, m)
                })
                .await;
            }
            "context-lifecycle" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::context_lifecycle::run(client, base_url, token, m)
                })
                .await;
            }
            "hook-track" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::hook_track::run(client, base_url, token, m)
                })
                .await;
            }
            "oauth-session" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::oauth_session::run(client, base_url, token, m)
                })
                .await;
            }
            "task-read" => {
                runner::run_scenario(&config, Arc::clone(&metrics), |client, base_url, token, m| {
                    scenarios::task_read::run(client, base_url, token, m)
                })
                .await;
            }
            "send-message" => {
                let agent_id = cli.agent_id.clone();
                runner::run_scenario(
                    &config,
                    Arc::clone(&metrics),
                    move |client, base_url, token, m| {
                        let agent_id = agent_id.clone();
                        async move {
                            scenarios::send_message::run(client, base_url, token, m, &agent_id)
                                .await;
                        }
                    },
                )
                .await;
            }
            other => {
                eprintln!("Unknown scenario: {other}");
                eprintln!("Available: api-latency, agent-registry, context-lifecycle, hook-track, oauth-session, task-read, send-message, all");
                std::process::exit(1);
            }
        }

        report.add(scenario_name.to_string(), &metrics);
    }

    let passed = report.print(&config.thresholds);
    if !passed {
        std::process::exit(1);
    }
}
