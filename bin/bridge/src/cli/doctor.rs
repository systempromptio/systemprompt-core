use std::process::ExitCode;

use systemprompt_identifiers::{PluginId, SessionId};

use crate::auth::ChainError;
use crate::auth::plugin_oauth;
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
    checks.push(check_cowork_enable());
    checks.push(check_plugin_installation_preference());
    checks.push(check_hook_token_mint(&client).await);
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

// Why: catches the silent "plugin is on disk but Cowork never picked it up"
// state. With the org-provisioned filesystem path, the bridge's only Cowork-
// side write is the enable key in cowork_settings.json — if that line is
// missing, the auto-installed plugin stays disabled and the operator gets no
// other signal from the bridge logs that the sync was effectively a no-op.
fn check_cowork_enable() -> Check {
    use crate::config::paths;
    use crate::integration::cowork_plugins::{
        COWORK_SETTINGS_FILE, enabled_plugins_key, resolve_target,
    };
    const ORG_PROVISIONED: &str = "org-provisioned";
    let Some(target) = resolve_target() else {
        return Check::warn(
            "cowork enable",
            "no active Cowork session detected — open Claude Cowork at least once before sync",
        );
    };
    let settings = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    let key = enabled_plugins_key(paths::SYNTHETIC_PLUGIN_NAME, ORG_PROVISIONED);
    let Ok(text) = std::fs::read_to_string(&settings) else {
        return Check::warn(
            "cowork enable",
            format!(
                "{} not yet written — run `systemprompt-bridge sync`",
                settings.display()
            ),
        );
    };
    let enabled = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("enabledPlugins").cloned())
        .and_then(|v| v.get(&key).cloned())
        == Some(serde_json::Value::Bool(true));
    if enabled {
        Check::ok(
            "cowork enable",
            format!("{key} = true in {}", settings.display()),
        )
    } else {
        Check::fail(
            "cowork enable",
            format!(
                "{key} not set in {} — Cowork will not load the synced plugin",
                settings.display()
            ),
        )
    }
}

// Why: a synced plugin whose `plugin.json` lacks (or defaults) `installationPreference`
// produces Cowork's "Contact an organization owner to install connectors" tooltip
// under MDM + custom-gateway deployment. We always emit `"auto_install"` from the
// bridge (see write_synthetic_plugin); this check fails loudly if a future refactor
// drops it.
// Docs: https://claude.com/docs/cowork/3p/extensions
fn check_plugin_installation_preference() -> Check {
    use crate::config::paths;
    let Some(location) = paths::org_plugins_effective() else {
        return Check::warn(
            "plugin auto-install",
            "no org-plugins location resolvable",
        );
    };
    let plugin_json = location
        .path
        .join(paths::SYNTHETIC_PLUGIN_NAME)
        .join(".claude-plugin")
        .join("plugin.json");
    let Ok(text) = std::fs::read_to_string(&plugin_json) else {
        return Check::warn(
            "plugin auto-install",
            format!(
                "{} not present — run `systemprompt-bridge sync`",
                plugin_json.display()
            ),
        );
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Check::fail(
            "plugin auto-install",
            format!("{}: invalid JSON", plugin_json.display()),
        );
    };
    let pref = value.get("installationPreference").and_then(|v| v.as_str());
    match pref {
        Some("required" | "auto_install") => Check::ok(
            "plugin auto-install",
            format!(
                "{} has installationPreference={}",
                plugin_json.display(),
                pref.unwrap_or(""),
            ),
        ),
        Some("available") => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference=available — Cowork will require a manual install \
                 click, which surfaces \"Contact an organization owner\" under MDM",
                plugin_json.display(),
            ),
        ),
        Some(other) => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference={other} is not one of required|auto_install|available",
                plugin_json.display(),
            ),
        ),
        None => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference is missing — Cowork will default to \"available\" \
                 (manual install, owner-gated)",
                plugin_json.display(),
            ),
        ),
    }
}

// Why: hook-token mint is the gateway-side step that fails silently as a
// host_failures row and only surfaces in `sync` PARTIAL output. Exercising it
// here turns the OAuth scope / policy errors into a single doctor line with the
// gateway's `error_description` verbatim, instead of waiting for the next sync.
async fn check_hook_token_mint(gateway: &GatewayClient) -> Check {
    let creds = match plugin_oauth::load_creds() {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Check::warn(
                "hook token mint",
                "no bridge OAuth client provisioned yet — runs on first sync after login",
            );
        },
        Err(e) => {
            return Check::fail("hook token mint", format!("load OAuth client creds: {e}"));
        },
    };
    let plugin_id = PluginId::new("__doctor__");
    match gateway
        .mint_plugin_hook_token(
            &creds.token_endpoint,
            &creds.client_id,
            &creds.client_secret,
            &plugin_id,
        )
        .await
    {
        Ok(_) => Check::ok(
            "hook token mint",
            format!(
                "{} accepted hook:govern hook:track for client {}",
                creds.token_endpoint,
                creds.client_id.as_str()
            ),
        ),
        Err(GatewayError::HookTokenRejected { status, body }) => Check::fail(
            "hook token mint",
            format!(
                "gateway rejected hook token: status={status} body={body} — operator action: \
                 confirm the bridge OAuth client grants `hook:govern hook:track` and that \
                 service-tier scopes are not being intersected with owner roles",
            ),
        ),
        Err(e) => Check::fail("hook token mint", format!("mint failed: {e}")),
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
