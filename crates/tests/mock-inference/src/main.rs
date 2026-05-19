//! Mock OpenAI/Anthropic-compatible inference endpoint for air-gapped load
//! testing.
//!
//! Serves deterministic responses on `POST /messages` (Anthropic Messages
//! wire format) and `POST /chat/completions` (OpenAI Chat wire format), with
//! configurable latency, failure injection, and degradation modes so latency
//! is reproducible across runs.

mod anthropic;
mod openai;
mod sse;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::routing::{get, post};
use clap::{Parser, ValueEnum};
use rand::Rng;
use serde_json::Value;

// Fixed count keeps body size — and therefore serialisation latency —
// reproducible.
pub const FIXED_OUTPUT_TOKENS: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    Ok,
    Timeout,
    #[value(name = "5xx")]
    FiveXx,
    SlowLoris,
}

#[derive(Debug, Parser)]
#[command(
    name = "mock-inference",
    about = "Mock inference endpoint for load testing"
)]
struct Cli {
    #[arg(long, default_value_t = 9100)]
    port: u16,

    // Accepts a fixed `N` or a `min:max` jitter range, both in milliseconds.
    #[arg(long, default_value = "0")]
    latency_ms: String,

    #[arg(long, default_value_t = 0.0)]
    fail_rate: f64,

    #[arg(long, value_enum, default_value_t = Mode::Ok)]
    mode: Mode,
}

#[derive(Debug, Clone, Copy)]
pub enum Latency {
    Fixed(u64),
    Range { min: u64, max: u64 },
}

impl Latency {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        if let Some((lo, hi)) = raw.split_once(':') {
            let min: u64 = lo.trim().parse().context("invalid latency min")?;
            let max: u64 = hi.trim().parse().context("invalid latency max")?;
            anyhow::ensure!(min <= max, "latency min must be <= max");
            Ok(Self::Range { min, max })
        } else {
            Ok(Self::Fixed(
                raw.trim().parse().context("invalid latency value")?,
            ))
        }
    }

    fn pick(self) -> Duration {
        let ms = match self {
            Self::Fixed(n) => n,
            Self::Range { min, max } if min == max => min,
            Self::Range { min, max } => rand::thread_rng().gen_range(min..=max),
        };
        Duration::from_millis(ms)
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub latency: Latency,
    pub fail_rate: f64,
    pub mode: Mode,
    // The air-gap egress proof reads this (via `GET /stats`) to assert e.g.
    // policy-denied requests never reach here.
    pub requests: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl AppState {
    pub fn note_request(&self, route: &str, model: &str) {
        let n = self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        tracing::info!(route, model, count = n, "inference request served");
    }

    pub async fn apply_latency(&self) {
        if matches!(self.mode, Mode::Timeout) {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            return;
        }
        let delay = self.latency.pick();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }

    pub fn should_fail(&self) -> bool {
        if matches!(self.mode, Mode::FiveXx) {
            return true;
        }
        self.fail_rate > 0.0 && rand::thread_rng().gen_bool(self.fail_rate.clamp(0.0, 1.0))
    }
}

pub fn count_input_tokens(body: &Value) -> u32 {
    let mut chars = 0usize;
    collect_text_len(body, &mut chars);
    ((chars / 4) + 1) as u32
}

fn collect_text_len(value: &Value, acc: &mut usize) {
    match value {
        Value::String(s) => *acc += s.chars().count(),
        Value::Array(arr) => arr.iter().for_each(|v| collect_text_len(v, acc)),
        Value::Object(map) => map.values().for_each(|v| collect_text_len(v, acc)),
        _ => {},
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let state = Arc::new(AppState {
        latency: Latency::parse(&cli.latency_ms)?,
        fail_rate: cli.fail_rate.clamp(0.0, 1.0),
        mode: cli.mode,
        requests: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
    });

    let app = Router::new()
        .route("/messages", post(anthropic::handle))
        .route("/chat/completions", post(openai::handle))
        .route("/health", get(|| async { "ok" }))
        .route(
            "/stats",
            get({
                let state = Arc::clone(&state);
                move || {
                    let n = state.requests.load(std::sync::atomic::Ordering::Relaxed);
                    async move { axum::Json(serde_json::json!({ "requests": n })) }
                }
            }),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    tracing::info!(%addr, mode = ?cli.mode, fail_rate = cli.fail_rate, "mock-inference listening");
    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
