//! `update` command: check for, and install, a newer bridge build.
//!
//! The only update path on Linux, where the bridge is CLI-only, and a
//! scriptable equivalent of the GUI button elsewhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{IsTerminal as _, Write as _};
use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::context::BridgeContext;
use crate::gateway::GatewayClient;
use crate::stdio::diag;
use crate::update::{self, UpdateStatus};
use crate::{auth, config, stdio};

// Why: a distinct exit code makes `--check` usable as a cron or
// config-management probe.
const EXIT_UPDATE_AVAILABLE: u8 = 1;

#[doc(hidden)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Args {
    pub check_only: bool,
    pub assume_yes: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ArgsError {
    #[error("unknown flag for `update`: {0}")]
    UnknownFlag(String),
}

#[doc(hidden)]
pub fn parse(argv: &[String]) -> Result<Args, ArgsError> {
    let mut args = Args::default();
    for arg in argv.iter().skip(2) {
        match arg.as_str() {
            "--check" => args.check_only = true,
            "--yes" | "-y" => args.assume_yes = true,
            other => return Err(ArgsError::UnknownFlag(other.to_owned())),
        }
    }
    Ok(args)
}

pub fn cmd_update(ctx: &BridgeContext, argv: &[String]) -> ExitCode {
    let args = match parse(argv) {
        Ok(a) => a,
        Err(e) => {
            diag(&e.to_string());
            return ExitCode::from(64);
        },
    };

    ctx.block_on(async move { run(ctx, &args).await })
}

async fn run(ctx: &BridgeContext, args: &Args) -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let bearer = match auth::acquire_bearer(&cfg, &SessionId::generate(), &ctx.http).await {
        Ok(out) => out,
        Err(ChainError::PreferredTransient { provider, source }) => {
            diag(&format!(
                "transient auth failure on preferred provider {provider}: {source}"
            ));
            return ExitCode::from(10);
        },
        Err(ChainError::NoneSucceeded) => {
            diag(&format!(
                "no credential available; run `{} login` first",
                crate::brand::brand().binary_name
            ));
            return ExitCode::from(5);
        },
    };

    let client = ctx.gateway_client(gateway.clone());
    let (status, manifest) = match update::check(&client, bearer.token.expose()).await {
        Ok(pair) => pair,
        Err(e) => {
            diag(&format!("update check failed: {e}"));
            return ExitCode::from(3);
        },
    };

    let brand = crate::brand::brand();
    match status {
        UpdateStatus::Current { version } => {
            stdio::print_line(&format!("{} {version} is up to date", brand.binary_name));
            ExitCode::SUCCESS
        },
        UpdateStatus::Available { version, notes_url } => {
            stdio::print_line(&format!(
                "{} {} is available (installed: {})",
                brand.binary_name, version, brand.version
            ));
            if let Some(url) = notes_url.as_deref() {
                stdio::print_line(&format!("release notes: {url}"));
            }
            if args.check_only {
                return ExitCode::from(EXIT_UPDATE_AVAILABLE);
            }
            if !args.assume_yes && !confirm(&version) {
                stdio::print_line("cancelled");
                return ExitCode::SUCCESS;
            }
            install(&client, bearer.token.expose(), &manifest, &version).await
        },
    }
}

async fn install(
    client: &GatewayClient,
    bearer: &str,
    manifest: &crate::gateway::types::ReleaseManifest,
    version: &str,
) -> ExitCode {
    let progress = progress_reporter();
    match update::apply(client, bearer, manifest, progress.as_ref()).await {
        Ok(path) => {
            stdio::print_line(&format!(
                "updated to {version} — {} (restart to run it)",
                path.display()
            ));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("update failed: {e}"));
            ExitCode::from(3)
        },
    }
}

// Why: silent when stderr is redirected — a progress bar in a CI log or a cron
// mail is noise.
fn progress_reporter() -> Box<dyn Fn(update::DownloadProgress) + Send + Sync> {
    if !std::io::stderr().is_terminal() {
        return Box::new(|_| {});
    }
    let last = std::sync::atomic::AtomicU8::new(u8::MAX);
    Box::new(move |p| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "fraction() is clamped to 0.0..=1.0, so the product fits u8"
        )]
        let pct = (p.fraction() * 100.0) as u8;
        if last.swap(pct, std::sync::atomic::Ordering::Relaxed) == pct {
            return;
        }
        let mut err = std::io::stderr();
        _ = write!(err, "\rdownloading… {pct:>3}%");
        if pct == 100 {
            _ = writeln!(err);
        }
        _ = err.flush();
    })
}

fn confirm(version: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        diag("not a terminal; re-run with --yes to install unattended");
        return false;
    }
    stdio::print_str(&format!("install {version}? [y/N] "));
    _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
