use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_analytics::matches_bot_pattern;

const DATACENTER_IP_PREFIXES: &[&str] = &[
    "47.79.", "47.82.", "47.88.", "47.89.", "47.90.", "47.91.", "47.92.", "47.93.", "47.94.",
    "47.95.", "47.96.", "47.97.", "47.98.", "47.99.", "47.100.", "47.101.", "47.102.", "47.103.",
    "47.104.", "47.105.", "47.106.", "47.107.", "47.108.", "47.109.", "47.110.", "47.111.",
    "47.112.", "47.113.", "47.114.", "47.115.", "47.116.", "47.117.", "47.118.", "47.119.",
    "119.29.", "129.28.", "162.14.", "119.3.", "122.112.",
];

const CHROME_MIN_VERSION: i32 = 120;

#[derive(Clone, Debug)]
pub struct BotMarker {
    pub is_bot: bool,
    pub bot_type: BotType,
    pub user_agent: String,
    pub ip_address: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotType {
    KnownBot,
    Scanner,
    Suspicious,
    Human,
}

pub async fn detect_bots_early(mut req: Request, next: Next) -> Response {
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| {
            h.to_str()
                .map_err(|e| {
                    tracing::trace!(error = %e, "Invalid UTF-8 in user-agent header");
                    e
                })
                .ok()
        })
        .unwrap_or("")
        .to_string();

    let ip_address = extract_ip_address(&req);
    let uri_path = req.uri().path().to_string();

    let marker = if is_known_bot(&user_agent) {
        BotMarker {
            is_bot: true,
            bot_type: BotType::KnownBot,
            user_agent: user_agent.clone(),
            ip_address: ip_address.clone(),
        }
    } else if is_datacenter_ip(ip_address.as_deref()) || is_outdated_browser(&user_agent) {
        BotMarker {
            is_bot: true,
            bot_type: BotType::Suspicious,
            user_agent: user_agent.clone(),
            ip_address: ip_address.clone(),
        }
    } else if is_scanner_request(&uri_path, &user_agent) {
        BotMarker {
            is_bot: false,
            bot_type: BotType::Scanner,
            user_agent: user_agent.clone(),
            ip_address: ip_address.clone(),
        }
    } else {
        BotMarker {
            is_bot: false,
            bot_type: BotType::Human,
            user_agent: user_agent.clone(),
            ip_address,
        }
    };

    req.extensions_mut().insert(Arc::new(marker));
    next.run(req).await
}

fn extract_ip_address(req: &Request) -> Option<String> {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        })
        .or_else(|| {
            req.headers()
                .get("cf-connecting-ip")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        })
}

fn is_datacenter_ip(ip: Option<&str>) -> bool {
    ip.is_some_and(|ip_addr| {
        DATACENTER_IP_PREFIXES
            .iter()
            .any(|prefix| ip_addr.starts_with(prefix))
    })
}

fn is_known_bot(user_agent: &str) -> bool {
    matches_bot_pattern(user_agent)
}

fn is_outdated_browser(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_lowercase();

    if let Some(pos) = ua_lower.find("chrome/") {
        let version_str = &ua_lower[pos + 7..];
        if let Some(dot_pos) = version_str.find('.') {
            if let Ok(major) = version_str[..dot_pos].parse::<i32>() {
                return major < CHROME_MIN_VERSION;
            }
        }
    }

    false
}

fn is_scanner_request(path: &str, user_agent: &str) -> bool {
    let scanner_paths = [
        ".env",
        ".git",
        ".php",
        "admin",
        "wp-admin",
        "wp-login",
        "administrator",
        ".sql",
        ".backup",
        "config.php",
        "web.config",
        ".well-known",
    ];

    let scanner_agents = [
        "masscan",
        "nmap",
        "nikto",
        "sqlmap",
        "metasploit",
        "nessus",
        "openvas",
        "zap",
        "burp",
        "qualys",
    ];

    let path_lower = path.to_lowercase();
    let ua_lower = user_agent.to_lowercase();

    scanner_paths.iter().any(|p| path_lower.contains(p))
        || scanner_agents.iter().any(|a| ua_lower.contains(a))
}
