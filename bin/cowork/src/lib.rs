pub mod cache;
pub mod config;
pub mod http;
pub mod install;
pub mod keystore;
pub mod loopback;
pub mod manifest;
pub mod output;
pub mod paths;
pub mod providers;
pub mod schedule;
pub mod setup;
pub mod sync;
pub mod types;
pub mod validate;

use std::env;
use std::process::ExitCode;

use crate::output::{diag, emit};
use crate::providers::mtls::MtlsProvider;
use crate::providers::pat::PatProvider;
use crate::providers::session::SessionProvider;
use crate::providers::{AuthError, AuthProvider};
use crate::schedule::Os;

const HELP: &str = "systemprompt-cowork <command>

Commands (credential helper):
  run                        (default) Emit JWT envelope to stdout
  login <sp-live-...>        Store a PAT securely and wire up systemprompt-cowork.toml
    [--gateway <url>]
  logout                     Remove the stored PAT and its config section
  status                     Show config paths and what is currently set up
  whoami                     Print authenticated identity from the gateway

Commands (plugin + MCP sync):
  install                    Bootstrap Cowork integration on this machine
    [--gateway <url>]                     Persist gateway URL + pin signing pubkey
    [--apply]                             Apply locally (Windows registry / macOS
                                          Managed Preferences direct-write). No MDM
                                          needed — works for a single-user dev setup.
    [--apply-mobileconfig]                (macOS) Build .mobileconfig and open System
                                          Settings → Profiles for user approval.
                                          Use when the fleet is MDM-managed or Apple's
                                          approval UI is required.
    [--print-mdm macos|windows|linux]     Print MDM snippet for target OS (default: current OS)
    [--emit-schedule-template macos|windows|linux]
                                          Write an OS scheduler template to CWD
    [--no-pubkey-fetch]                   Skip live fetch of manifest signing pubkey
  sync                       Pull plugins + MCP allowlist from gateway into org-plugins
    [--watch] [--interval <secs>] [--allow-unsigned]
  validate                   End-to-end self-check (paths, gateway, creds, signatures)
  uninstall                  Reverse install (metadata + staging)
    [--purge]                             Also remove stored PAT/credentials
  help                       Show this help

Env overrides:
  SP_COWORK_CONFIG           Path to systemprompt-cowork.toml
  SP_COWORK_PAT              Inline PAT (overrides file-based [pat])
  SP_COWORK_GATEWAY_URL      Override gateway_url
";

pub fn run() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        None | Some("run") => dispatch_run(),
        Some("login") => dispatch_login(&args),
        Some("logout") => dispatch_logout(),
        Some("status") => dispatch_status(),
        Some("whoami") => dispatch_whoami(),
        Some("install") => dispatch_install(&args),
        Some("sync") => dispatch_sync(&args),
        Some("validate") => validate::validate(),
        Some("uninstall") => dispatch_uninstall(&args),
        Some("help" | "--help" | "-h") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        },
        Some(other) => {
            diag(&format!("unknown command: {other}"));
            eprint!("{HELP}");
            ExitCode::from(64)
        },
    }
}

fn dispatch_run() -> ExitCode {
    if let Some(cached) = cache::read_valid() {
        if emit(&cached).is_err() {
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }

    let cfg = config::load();

    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(MtlsProvider::new(&cfg)),
        Box::new(SessionProvider::new(&cfg)),
        Box::new(PatProvider::new(&cfg)),
    ];

    for provider in &chain {
        match provider.authenticate() {
            Ok(out) => {
                if let Err(e) = cache::write(&out) {
                    diag(&format!("cache write failed (continuing): {e}"));
                }
                if emit(&out).is_err() {
                    return ExitCode::from(2);
                }
                return ExitCode::SUCCESS;
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(msg)) => {
                diag(&format!("{}: {msg}", provider.name()));
                continue;
            },
        }
    }

    diag("no credential source succeeded");
    diag("run `systemprompt-cowork login <sp-live-...>` to configure a PAT");
    ExitCode::from(5)
}

fn dispatch_login(args: &[String]) -> ExitCode {
    let token = match args.get(2) {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            diag("usage: systemprompt-cowork login <sp-live-...> [--gateway <url>]");
            return ExitCode::from(64);
        },
    };
    let gateway = parse_opt_flag(args, "--gateway");

    match setup::login(&token, gateway.as_deref()) {
        Ok(paths) => {
            println!("Stored PAT for systemprompt-cowork helper.");
            println!("  config: {}", paths.config_file.display());
            println!("  secret: {} (0600)", paths.pat_file.display());
            println!("Next: run `systemprompt-cowork` to fetch a JWT.");
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn dispatch_logout() -> ExitCode {
    match setup::logout() {
        Ok(paths) => {
            println!("Removed PAT.");
            println!("  config: {}", paths.config_file.display());
            println!("  secret: {}", paths.pat_file.display());
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("logout failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn dispatch_status() -> ExitCode {
    match setup::status() {
        Ok(s) => {
            println!("config file: {}", s.paths.config_file.display());
            println!("  present: {}", s.config_present);
            println!("secret file: {}", s.paths.pat_file.display());
            println!("  present: {}", s.pat_present);
            if let Some(loc) = paths::org_plugins_effective() {
                println!("org-plugins: {}", loc.path.display());
                let meta = paths::metadata_dir(&loc.path);
                let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
                println!(
                    "  last sync: {}",
                    if last_sync.exists() {
                        last_sync.display().to_string()
                    } else {
                        "(never)".into()
                    }
                );
                let user_file = meta.join(paths::USER_FRAGMENT);
                if let Ok(bytes) = std::fs::read(&user_file) {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(email) = value.get("email").and_then(|v| v.as_str()) {
                            println!("  identity: {email}");
                        }
                    }
                }
                let skills_idx = meta.join(paths::SKILLS_DIR).join("index.json");
                if let Ok(bytes) = std::fs::read(&skills_idx) {
                    if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(n) = arr.as_array().map(|a| a.len()) {
                            println!("  skills: {n}");
                        }
                    }
                }
                let agents_idx = meta.join(paths::AGENTS_DIR).join("index.json");
                if let Ok(bytes) = std::fs::read(&agents_idx) {
                    if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(n) = arr.as_array().map(|a| a.len()) {
                            println!("  agents: {n}");
                        }
                    }
                }
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("status failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn dispatch_whoami() -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let bearer = match cache::read_valid() {
        Some(out) => out.token,
        None => {
            let chain: Vec<Box<dyn AuthProvider>> = vec![
                Box::new(MtlsProvider::new(&cfg)),
                Box::new(SessionProvider::new(&cfg)),
                Box::new(PatProvider::new(&cfg)),
            ];
            let mut token = None;
            for p in &chain {
                match p.authenticate() {
                    Ok(out) => {
                        let _ = cache::write(&out);
                        token = Some(out.token);
                        break;
                    },
                    Err(AuthError::NotConfigured) => continue,
                    Err(AuthError::Failed(msg)) => {
                        diag(&format!("{}: {msg}", p.name()));
                    },
                }
            }
            match token {
                Some(t) => t,
                None => {
                    diag("no credential available; run `systemprompt-cowork login` first");
                    return ExitCode::from(5);
                },
            }
        },
    };

    let client = http::GatewayClient::new(gateway);
    match client.fetch_whoami(&bearer) {
        Ok(value) => {
            match serde_json::to_string_pretty(&value) {
                Ok(s) => println!("{s}"),
                Err(_) => println!("{value}"),
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("whoami failed: {e}"));
            ExitCode::from(3)
        },
    }
}

fn dispatch_install(args: &[String]) -> ExitCode {
    let print_mdm = parse_opt_flag(args, "--print-mdm")
        .as_deref()
        .and_then(Os::parse);
    let emit_sched = parse_opt_flag(args, "--emit-schedule-template")
        .as_deref()
        .and_then(Os::parse);
    let gateway = parse_opt_flag(args, "--gateway");
    let no_pubkey_fetch = has_flag(args, "--no-pubkey-fetch");
    let apply = has_flag(args, "--apply");
    let apply_mobileconfig = has_flag(args, "--apply-mobileconfig");
    install::install(install::InstallOptions {
        print_mdm,
        emit_schedule_template: emit_sched,
        gateway_url: gateway,
        no_pubkey_fetch,
        apply,
        apply_mobileconfig,
    })
}

fn dispatch_sync(args: &[String]) -> ExitCode {
    let watch = has_flag(args, "--watch");
    let interval = parse_opt_flag(args, "--interval").and_then(|s| s.parse().ok());
    let allow_unsigned = has_flag(args, "--allow-unsigned");
    sync::sync(sync::SyncOptions {
        watch,
        interval,
        allow_unsigned,
    })
}

fn dispatch_uninstall(args: &[String]) -> ExitCode {
    let purge = has_flag(args, "--purge");
    install::uninstall(purge)
}

fn parse_opt_flag(args: &[String], flag: &str) -> Option<String> {
    let mut i = 2;
    while i < args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().skip(2).any(|a| a == flag)
}
