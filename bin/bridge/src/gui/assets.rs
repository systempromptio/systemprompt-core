//! Embedded GUI web assets: HTML shell, stylesheets, and fonts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

// All web assets are `include_str!`d from `$OUT_DIR/web`, where the build
// script stages core's `web/` tree and applies an optional brand overlay
// (`SYSTEMPROMPT_BRIDGE_WEB_OVERLAY`) on top. A white-label repo overrides any
// of these files by shipping its own copy in that overlay dir. See build.rs.
macro_rules! web_asset {
    ($path:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/web/", $path))
    };
}

const HTML: &str = web_asset!("index.html");

const CSS_FILES: &[(&str, &str)] = &[
    ("tokens", web_asset!("css/tokens.css")),
    ("fonts", web_asset!("css/fonts.css")),
    ("reset", web_asset!("css/reset.css")),
    ("kbd", web_asset!("css/kbd.css")),
    ("dot", web_asset!("css/dot.css")),
    ("badge", web_asset!("css/badge.css")),
    ("button", web_asset!("css/button.css")),
    ("topbar", web_asset!("css/topbar.css")),
    ("rail", web_asset!("css/rail.css")),
    ("shell", web_asset!("css/shell.css")),
    ("drawer", web_asset!("css/drawer.css")),
    ("marketplace-base", web_asset!("css/marketplace-base.css")),
    ("marketplace-list", web_asset!("css/marketplace-list.css")),
    (
        "marketplace-detail",
        web_asset!("css/marketplace-detail.css"),
    ),
    ("status", web_asset!("css/status.css")),
    ("settings", web_asset!("css/settings.css")),
    ("setup", web_asset!("css/setup.css")),
    ("agents", web_asset!("css/agents.css")),
    ("profile", web_asset!("css/profile.css")),
    ("log", web_asset!("css/log.css")),
    ("footer", web_asset!("css/footer.css")),
    ("responsive", web_asset!("css/responsive.css")),
    ("toast", web_asset!("css/toast.css")),
    ("brand-overrides", web_asset!("css/brand-overrides.css")),
    ("main", web_asset!("css/main.css")),
];

const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.woff2");
const FONT_INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.woff2");
const FONT_OPENSANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.woff2");
const FONT_OPENSANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Bold.woff2");

const JS_MODULES: &[(&str, &str)] = &[
    ("i18n", web_asset!("js/i18n.js")),
    ("theme", web_asset!("js/theme.js")),
    ("bridge", web_asset!("js/bridge.js")),
    ("index", web_asset!("js/index.js")),
    (
        "events/bridge-events",
        web_asset!("js/events/bridge-events.js"),
    ),
    (
        "services/marketplace-service",
        web_asset!("js/services/marketplace-service.js"),
    ),
    ("utils/rail-tabs", web_asset!("js/utils/rail-tabs.js")),
    ("utils/gateway", web_asset!("js/utils/gateway.js")),
    ("utils/format", web_asset!("js/utils/format.js")),
    (
        "components/log-virtual",
        web_asset!("js/components/log-virtual.js"),
    ),
    (
        "components/sp-element",
        web_asset!("js/components/sp-element.js"),
    ),
    (
        "components/sp-cloud-status",
        web_asset!("js/components/sp-cloud-status.js"),
    ),
    (
        "components/sp-proxy-status",
        web_asset!("js/components/sp-proxy-status.js"),
    ),
    (
        "components/sp-mcp-auth-status",
        web_asset!("js/components/sp-mcp-auth-status.js"),
    ),
    (
        "components/sp-agent-presence",
        web_asset!("js/components/sp-agent-presence.js"),
    ),
    (
        "components/sp-agents-summary",
        web_asset!("js/components/sp-agents-summary.js"),
    ),
    (
        "components/sp-agents-status",
        web_asset!("js/components/sp-agents-status.js"),
    ),
    (
        "components/sp-overall-badge",
        web_asset!("js/components/sp-overall-badge.js"),
    ),
    (
        "components/sp-sync-pill",
        web_asset!("js/components/sp-sync-pill.js"),
    ),
    (
        "components/sp-rail-profile",
        web_asset!("js/components/sp-rail-profile.js"),
    ),
    (
        "components/sp-footer",
        web_asset!("js/components/sp-footer.js"),
    ),
    (
        "components/sp-crumb",
        web_asset!("js/components/sp-crumb.js"),
    ),
    ("components/sp-rail", web_asset!("js/components/sp-rail.js")),
    (
        "components/sp-toast",
        web_asset!("js/components/sp-toast.js"),
    ),
    (
        "components/sp-activity-log",
        web_asset!("js/components/sp-activity-log.js"),
    ),
    (
        "components/sp-host-card",
        web_asset!("js/components/sp-host-card.js"),
    ),
    (
        "components/sp-hosts-list",
        web_asset!("js/components/sp-hosts-list.js"),
    ),
    (
        "components/sp-settings",
        web_asset!("js/components/sp-settings.js"),
    ),
    (
        "components/sp-profile",
        web_asset!("js/components/sp-profile.js"),
    ),
    (
        "components/sp-marketplace",
        web_asset!("js/components/sp-marketplace.js"),
    ),
    (
        "components/sp-marketplace-list",
        web_asset!("js/components/sp-marketplace-list.js"),
    ),
    (
        "components/sp-marketplace-detail",
        web_asset!("js/components/sp-marketplace-detail.js"),
    ),
    (
        "components/sp-setup",
        web_asset!("js/components/sp-setup.js"),
    ),
    (
        "components/sp-setup-gateway",
        web_asset!("js/components/sp-setup-gateway.js"),
    ),
    (
        "components/sp-setup-agents",
        web_asset!("js/components/sp-setup-agents.js"),
    ),
];

const I18N_FILES: &[(&str, &str)] = &[("en-US/bridge", web_asset!("i18n/en-US/bridge.ftl"))];

const VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_SHA_FULL: &str = env!("VERGEN_GIT_SHA");
const BUILD_DATE: &str = env!("VERGEN_GIT_COMMIT_DATE");

fn git_sha_short() -> &'static str {
    let len = GIT_SHA_FULL.len().min(7);
    &GIT_SHA_FULL[..len]
}

pub const PLATFORM_SLUG: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

pub const PLATFORM_DISPLAY: &str = if cfg!(target_os = "macos") {
    "macOS"
} else if cfg!(target_os = "windows") {
    "Windows"
} else {
    "Linux"
};

#[derive(Debug)]
pub struct Asset {
    pub content_type: &'static str,
    pub body: Cow<'static, [u8]>,
}

impl Asset {
    const fn text(content_type: &'static str, body: String) -> Self {
        Self {
            content_type,
            body: Cow::Owned(body.into_bytes()),
        }
    }

    const fn raw(content_type: &'static str, body: &'static [u8]) -> Self {
        Self {
            content_type,
            body: Cow::Borrowed(body),
        }
    }
}

pub fn render_index() -> String {
    let brand = crate::brand::brand();
    let html = HTML
        .replace("__VERSION__", VERSION)
        .replace("__GIT_SHA__", git_sha_short())
        .replace("__BUILD_DATE__", BUILD_DATE)
        .replace("__ICON_SVG__", brand.assets.icon_svg)
        .replace("__LOGO_SVG__", brand.assets.logo_svg)
        .replace("__PLATFORM_DISPLAY__", PLATFORM_DISPLAY)
        .replace("__PLATFORM__", PLATFORM_SLUG);
    // Append the brand theme override last so its `:root { --sp-* }` wins the
    // cascade over the bundled token sheet.
    if brand.assets.theme_css.is_empty() {
        html
    } else {
        let style = format!(
            "<style id=\"brand-theme\">{}</style>",
            brand.assets.theme_css
        );
        html.replacen("</head>", &format!("{style}</head>"), 1)
    }
}

pub fn lookup_path(path: &str) -> Option<Asset> {
    if path == "/" || path == "/index.html" {
        return Some(Asset::text("text/html; charset=utf-8", render_index()));
    }
    if let Some(name) = path
        .strip_prefix("/assets/css/")
        .and_then(|s| s.strip_suffix(".css"))
        && let Some((_, src)) = CSS_FILES.iter().find(|(n, _)| *n == name)
    {
        return Some(Asset::text("text/css; charset=utf-8", (*src).to_owned()));
    }
    if let Some(name) = path
        .strip_prefix("/assets/js/")
        .and_then(|s| s.strip_suffix(".js"))
        && let Some((_, src)) = JS_MODULES.iter().find(|(n, _)| *n == name)
    {
        return Some(Asset::text(
            "application/javascript; charset=utf-8",
            (*src).to_owned(),
        ));
    }
    if let Some(name) = path
        .strip_prefix("/assets/i18n/")
        .and_then(|s| s.strip_suffix(".ftl"))
        && let Some((_, src)) = I18N_FILES.iter().find(|(n, _)| *n == name)
    {
        return Some(Asset::text("text/plain; charset=utf-8", (*src).to_owned()));
    }
    match path {
        "/assets/fonts/Inter-Regular.woff2" => Some(Asset::raw("font/woff2", FONT_INTER_REGULAR)),
        "/assets/fonts/Inter-Bold.woff2" => Some(Asset::raw("font/woff2", FONT_INTER_BOLD)),
        "/assets/fonts/OpenSans-Regular.woff2" => {
            Some(Asset::raw("font/woff2", FONT_OPENSANS_REGULAR))
        },
        "/assets/fonts/OpenSans-Bold.woff2" => Some(Asset::raw("font/woff2", FONT_OPENSANS_BOLD)),
        _ => None,
    }
}
