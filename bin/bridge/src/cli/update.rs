//! `update` command: check for, and install, a newer bridge build.
//!
//! The only update path on Linux, where the bridge is CLI-only, and a scriptable
//! equivalent of the GUI button elsewhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{IsTerminal as _, Write as _};
use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;
use crate::update::{self, UpdateStatus};
use crate::{auth, config};

// Why: a distinct exit code makes `--check` usable as a cron or
// config-management probe.
const EXIT_UPDATE_AVAILABLE: u8 = 1;

#[derive(Debug, Default)]
struct Args {
    check_only: bool,
    assume_yes: bool,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    for arg in argv.iter().skip(2) {
        match arg.as_str() {
            "--check" => args.check_only = true,
            "--yes" | "-y" => args.assume_yes = true,
            other => return Err(format!("unknown flag for `update`: {other}")),
        }
    }
    Ok(args)
}

pub fn cmd_update(argv: &[String]) -> ExitCode {
    let args = match parse(argv) {
        Ok(a) => a,
        Err(e) => {
            diag(&e);
            return ExitCode::from(64);
        },
    };

    match crate::proxy::block_on(async move { run(&args).await }) {
        Ok(code) => code,
        Err(e) => {
            diag(&format!("runtime init failed: {e}"));
            ExitCode::from(70)
        },
    }
}

async fn run(args: &Args) -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let bearer = match auth::acquire_bearer(&cfg, &SessionId::generate()).await {
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

    let client = GatewayClient::new(gateway.clone());
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
            output::print_line(&format!("{} {version} is up to date", brand.binary_name));
            ExitCode::SUCCESS
        },
        UpdateStatus::Available { version, notes_url } => {
            output::print_line(&format!(
                "{} {} is available (installed: {})",
                brand.binary_name, version, brand.version
            ));
            if let Some(url) = notes_url.as_deref() {
                output::print_line(&format!("release notes: {url}"));
            }
            if args.check_only {
                return ExitCode::from(EXIT_UPDATE_AVAILABLE);
            }
            if !args.assume_yes && !confirm(&version) {
                output::print_line("cancelled");
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
            output::print_line(&format!(
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
    output::print_str(&format!("install {version}? [y/N] "));
    _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(rest: &[&str]) -> Vec<String> {
        let mut v = vec!["bridge".to_owned(), "update".to_owned()];
        v.extend(rest.iter().map(|s| (*s).to_owned()));
        v
    }

    #[test]
    fn bare_update_installs_interactively() {
        let args = parse(&argv(&[]));
        assert!(matches!(
            args,
            Ok(Args {
                check_only: false,
                assume_yes: false
            })
        ));
    }

    #[test]
    fn flags_parse() {
        assert!(matches!(
            parse(&argv(&["--check"])),
            Ok(Args {
                check_only: true,
                ..
            })
        ));
        assert!(matches!(
            parse(&argv(&["-y"])),
            Ok(Args {
                assume_yes: true, ..
            })
        ));
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse(&argv(&["--force"])).is_err());
    }
}
