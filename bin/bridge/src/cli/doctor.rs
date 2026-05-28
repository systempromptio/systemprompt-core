use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::proxy::secret as proxy_secret;
use crate::{auth, config, obs};

enum Status {
    Ok,
    Warn,
    Fail,
}

struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Fail,
            detail: detail.into(),
        }
    }
}

pub(super) fn cmd_doctor() -> ExitCode {
    let result = crate::proxy::block_on(async { run_checks().await });
    match result {
        Ok((checks, any_fail)) => {
            render(&checks);
            if any_fail {
                ExitCode::from(11)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            obs::output::diag(&format!("doctor: runtime init failed: {e}"));
            ExitCode::from(70)
        },
    }
}

async fn run_checks() -> (Vec<Check>, bool) {
    let cfg = config::load();
    let mut checks: Vec<Check> = Vec::new();
    checks.push(check_config_file());
    checks.push(check_credential_source(&cfg));
    let bearer = check_mint_jwt(&cfg, &mut checks).await;
    let client = check_gateway_reachable(&cfg, &mut checks).await;
    check_whoami(&client, bearer.as_ref(), &mut checks).await;
    checks.push(check_loopback_secret());
    checks.push(check_pinned_pubkey());
    checks.push(check_cowork_marketplace());
    let any_fail = checks.iter().any(|c| matches!(c.status, Status::Fail));
    (checks, any_fail)
}

fn check_credential_source(cfg: &config::Config) -> Check {
    if auth::has_credential_source(cfg) {
        Check::ok(
            "credential source",
            "at least one auth provider is configured (PAT, session, or mTLS)",
        )
    } else {
        Check::fail(
            "credential source",
            "no auth provider configured — run `systemprompt-bridge login <sp-live-...>`",
        )
    }
}

async fn check_mint_jwt(
    cfg: &config::Config,
    checks: &mut Vec<Check>,
) -> Option<auth::types::HelperOutput> {
    let session_id = SessionId::generate();
    match auth::acquire_bearer(cfg, &session_id).await {
        Ok(out) => {
            checks.push(Check::ok(
                "mint JWT",
                "auth chain succeeded; gateway accepted credentials at token-exchange",
            ));
            Some(out)
        },
        Err(ChainError::PreferredTransient { provider, source }) => {
            checks.push(Check::fail(
                "mint JWT",
                format!("preferred provider `{provider}` failed transiently: {source}"),
            ));
            None
        },
        Err(ChainError::NoneSucceeded) => {
            checks.push(Check::fail(
                "mint JWT",
                "no provider in the chain succeeded — run `systemprompt-bridge login`",
            ));
            None
        },
    }
}

async fn check_gateway_reachable(cfg: &config::Config, checks: &mut Vec<Check>) -> GatewayClient {
    let gateway = config::gateway_url_or_default(cfg);
    let client = GatewayClient::new(gateway.clone());
    match client.health().await {
        Ok(()) => checks.push(Check::ok(
            "gateway reachable",
            format!("{} responds on /health", gateway.as_str()),
        )),
        Err(e) => checks.push(Check::fail(
            "gateway reachable",
            format!("{}: {e}", gateway.as_str()),
        )),
    }
    client
}

async fn check_whoami(
    client: &GatewayClient,
    bearer: Option<&auth::types::HelperOutput>,
    checks: &mut Vec<Check>,
) {
    let Some(out) = bearer else {
        checks.push(Check::fail(
            "authenticated whoami",
            "skipped: no bearer token available (see `mint JWT` above)",
        ));
        return;
    };
    match client.fetch_whoami(out.token.expose()).await {
        Ok(_) => checks.push(Check::ok(
            "authenticated whoami",
            "GET /v1/bridge/whoami returned identity",
        )),
        Err(GatewayError::HttpStatus { status, endpoint }) if status.as_u16() == 401 => {
            checks.push(Check::fail(
                "authenticated whoami",
                format!(
                    "{endpoint} returned 401 — the PAT is invalid or revoked; mint a new one and \
                     re-run `systemprompt-bridge login`"
                ),
            ));
        },
        Err(e) => checks.push(Check::fail(
            "authenticated whoami",
            format!("whoami failed: {e}"),
        )),
    }
}

fn check_loopback_secret() -> Check {
    let Some(path) = proxy_secret::secret_path() else {
        return Check::fail(
            "loopback secret",
            "no config dir resolvable (dirs::config_dir() returned None)",
        );
    };
    match proxy_secret::load(&path) {
        Ok(Some(_)) => Check::ok("loopback secret", format!("{} present", path.display())),
        Ok(None) => Check::warn(
            "loopback secret",
            format!(
                "{} not present — proxy will mint it on first start; restart Claude Desktop \
                 afterwards",
                path.display()
            ),
        ),
        Err(e) => Check::fail("loopback secret", format!("{}: {e}", path.display())),
    }
}

// Why: catches "publish() got far enough to copy plugin files but errored before
// registering the marketplace" — the exact silent-half-publish state that an NTFS
// path-segment error in cache_dir produced on Windows. If the bridge marketplace
// folder exists but known_marketplaces.json does not list it, Cowork's Directory
// UI shows "no plugins" with no other signal.
fn check_cowork_marketplace() -> Check {
    use crate::config::paths;
    use crate::integration::cowork_plugins::{
        KNOWN_MARKETPLACES_FILE, KnownMarketplacesFile, resolve_target,
    };
    let Some(target) = resolve_target() else {
        return Check::warn(
            "cowork marketplace",
            "no active Cowork session detected — open Claude Cowork at least once before sync",
        );
    };
    let mp_dir = target
        .cowork_plugins_dir
        .join("marketplaces")
        .join(paths::BRIDGE_MARKETPLACE_NAME);
    let known = target.cowork_plugins_dir.join(KNOWN_MARKETPLACES_FILE);
    let mp_dir_exists = mp_dir.is_dir();
    let registered = match std::fs::read_to_string(&known) {
        Ok(text) => serde_json::from_str::<KnownMarketplacesFile>(&text)
            .map(|f| f.contains(paths::BRIDGE_MARKETPLACE_NAME))
            .unwrap_or(false),
        Err(_) => false,
    };
    match (mp_dir_exists, registered) {
        (true, true) => Check::ok(
            "cowork marketplace",
            format!("{} registered in {}", paths::BRIDGE_MARKETPLACE_NAME, known.display()),
        ),
        (true, false) => Check::fail(
            "cowork marketplace",
            format!(
                "publish partial — {} exists but {} does not list it; see bridge.log for the \
                 underlying host-sync error (likely an emit IO failure)",
                mp_dir.display(),
                known.display(),
            ),
        ),
        (false, _) => Check::warn(
            "cowork marketplace",
            "marketplace dir not yet written — run `systemprompt-bridge sync`",
        ),
    }
}

fn check_pinned_pubkey() -> Check {
    if config::pinned_pubkey().is_some() {
        Check::ok(
            "manifest pubkey pinned",
            "signed-manifest verification will reject pubkey rotation",
        )
    } else {
        Check::warn(
            "manifest pubkey pinned",
            "no pinned pubkey — first sync needs `--allow-tofu` or `install --apply --pubkey \
             <b64>`",
        )
    }
}

fn check_config_file() -> Check {
    let Some(path) = config::config_path() else {
        return Check::fail("config file", "no config dir resolvable");
    };
    if !path.exists() {
        return Check::warn(
            "config file",
            format!(
                "{} not present — defaults will be used; run `systemprompt-bridge login` to \
                 create it",
                path.display()
            ),
        );
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<toml::Value>(&text) {
            Ok(_) => Check::ok("config file", format!("{} parses cleanly", path.display())),
            Err(e) => Check::fail(
                "config file",
                format!("{}: parse error: {e}", path.display()),
            ),
        },
        Err(e) => Check::fail("config file", format!("{}: {e}", path.display())),
    }
}

fn render(checks: &[Check]) {
    let mut buf = String::new();
    for c in checks {
        let tag = match c.status {
            Status::Ok => "OK  ",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        };
        buf.push_str(&format!("[{tag}] {:<28} {}\n", c.name, c.detail));
    }
    let fails = checks
        .iter()
        .filter(|c| matches!(c.status, Status::Fail))
        .count();
    let warns = checks
        .iter()
        .filter(|c| matches!(c.status, Status::Warn))
        .count();
    buf.push_str(&format!(
        "\nsummary: {} ok, {warns} warn, {fails} fail\n",
        checks.len() - fails - warns
    ));
    output::print_str(&buf);
}
