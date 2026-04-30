use std::borrow::Cow;

const HTML: &str = include_str!("../../web/index.html");

const CSS_FILES: &[(&str, &str)] = &[
    ("tokens", include_str!("../../web/css/tokens.css")),
    ("fonts", include_str!("../../web/css/fonts.css")),
    ("reset", include_str!("../../web/css/reset.css")),
    ("kbd", include_str!("../../web/css/kbd.css")),
    ("dot", include_str!("../../web/css/dot.css")),
    ("badge", include_str!("../../web/css/badge.css")),
    ("button", include_str!("../../web/css/button.css")),
    ("topbar", include_str!("../../web/css/topbar.css")),
    ("rail", include_str!("../../web/css/rail.css")),
    ("shell", include_str!("../../web/css/shell.css")),
    ("drawer", include_str!("../../web/css/drawer.css")),
    (
        "marketplace-base",
        include_str!("../../web/css/marketplace-base.css"),
    ),
    (
        "marketplace-list",
        include_str!("../../web/css/marketplace-list.css"),
    ),
    (
        "marketplace-detail",
        include_str!("../../web/css/marketplace-detail.css"),
    ),
    ("status", include_str!("../../web/css/status.css")),
    ("settings", include_str!("../../web/css/settings.css")),
    ("setup", include_str!("../../web/css/setup.css")),
    ("agents", include_str!("../../web/css/agents.css")),
    ("log", include_str!("../../web/css/log.css")),
    ("footer", include_str!("../../web/css/footer.css")),
    ("responsive", include_str!("../../web/css/responsive.css")),
    ("toast", include_str!("../../web/css/toast.css")),
    ("main", include_str!("../../web/css/main.css")),
];

const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.woff2");
const FONT_INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.woff2");
const FONT_OPENSANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.woff2");
const FONT_OPENSANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Bold.woff2");

const JS_MODULES: &[(&str, &str)] = &[
    ("i18n", include_str!("../../web/js/i18n.js")),
    ("theme", include_str!("../../web/js/theme.js")),
    ("bridge", include_str!("../../web/js/bridge.js")),
    ("index", include_str!("../../web/js/index.js")),
    (
        "events/bridge-events",
        include_str!("../../web/js/events/bridge-events.js"),
    ),
    (
        "services/marketplace-service",
        include_str!("../../web/js/services/marketplace-service.js"),
    ),
    (
        "utils/rail-tabs",
        include_str!("../../web/js/utils/rail-tabs.js"),
    ),
    (
        "utils/gateway",
        include_str!("../../web/js/utils/gateway.js"),
    ),
    (
        "components/log-virtual",
        include_str!("../../web/js/components/log-virtual.js"),
    ),
    (
        "components/sp-element",
        include_str!("../../web/js/components/sp-element.js"),
    ),
    (
        "components/sp-cloud-status",
        include_str!("../../web/js/components/sp-cloud-status.js"),
    ),
    (
        "components/sp-proxy-status",
        include_str!("../../web/js/components/sp-proxy-status.js"),
    ),
    (
        "components/sp-agent-presence",
        include_str!("../../web/js/components/sp-agent-presence.js"),
    ),
    (
        "components/sp-agents-summary",
        include_str!("../../web/js/components/sp-agents-summary.js"),
    ),
    (
        "components/sp-overall-badge",
        include_str!("../../web/js/components/sp-overall-badge.js"),
    ),
    (
        "components/sp-sync-pill",
        include_str!("../../web/js/components/sp-sync-pill.js"),
    ),
    (
        "components/sp-rail-profile",
        include_str!("../../web/js/components/sp-rail-profile.js"),
    ),
    (
        "components/sp-footer",
        include_str!("../../web/js/components/sp-footer.js"),
    ),
    (
        "components/sp-crumb",
        include_str!("../../web/js/components/sp-crumb.js"),
    ),
    (
        "components/sp-rail",
        include_str!("../../web/js/components/sp-rail.js"),
    ),
    (
        "components/sp-toast",
        include_str!("../../web/js/components/sp-toast.js"),
    ),
    (
        "components/sp-activity-log",
        include_str!("../../web/js/components/sp-activity-log.js"),
    ),
    (
        "components/sp-host-card",
        include_str!("../../web/js/components/sp-host-card.js"),
    ),
    (
        "components/sp-hosts-list",
        include_str!("../../web/js/components/sp-hosts-list.js"),
    ),
    (
        "components/sp-settings",
        include_str!("../../web/js/components/sp-settings.js"),
    ),
    (
        "components/sp-marketplace",
        include_str!("../../web/js/components/sp-marketplace.js"),
    ),
    (
        "components/sp-marketplace-list",
        include_str!("../../web/js/components/sp-marketplace-list.js"),
    ),
    (
        "components/sp-marketplace-detail",
        include_str!("../../web/js/components/sp-marketplace-detail.js"),
    ),
    (
        "components/sp-setup",
        include_str!("../../web/js/components/sp-setup.js"),
    ),
    (
        "components/sp-setup-gateway",
        include_str!("../../web/js/components/sp-setup-gateway.js"),
    ),
    (
        "components/sp-setup-agents",
        include_str!("../../web/js/components/sp-setup-agents.js"),
    ),
];

const I18N_FILES: &[(&str, &str)] = &[(
    "en-US/bridge",
    include_str!("../../web/i18n/en-US/bridge.ftl"),
)];

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

pub struct Asset {
    pub content_type: &'static str,
    pub body: Cow<'static, [u8]>,
}

impl Asset {
    fn text(content_type: &'static str, body: String) -> Self {
        Self {
            content_type,
            body: Cow::Owned(body.into_bytes()),
        }
    }

    fn raw(content_type: &'static str, body: &'static [u8]) -> Self {
        Self {
            content_type,
            body: Cow::Borrowed(body),
        }
    }
}

pub fn render_index() -> String {
    HTML.replace("__VERSION__", VERSION)
        .replace("__GIT_SHA__", git_sha_short())
        .replace("__BUILD_DATE__", BUILD_DATE)
        .replace("__ICON_SVG__", ICON_SVG)
        .replace("__LOGO_SVG__", LOGO_SVG)
        .replace("__PLATFORM_DISPLAY__", PLATFORM_DISPLAY)
        .replace("__PLATFORM__", PLATFORM_SLUG)
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
        return Some(Asset::text(
            "text/css; charset=utf-8",
            (*src).to_string(),
        ));
    }
    if let Some(name) = path
        .strip_prefix("/assets/js/")
        .and_then(|s| s.strip_suffix(".js"))
        && let Some((_, src)) = JS_MODULES.iter().find(|(n, _)| *n == name)
    {
        return Some(Asset::text(
            "application/javascript; charset=utf-8",
            (*src).to_string(),
        ));
    }
    if let Some(name) = path
        .strip_prefix("/assets/i18n/")
        .and_then(|s| s.strip_suffix(".ftl"))
        && let Some((_, src)) = I18N_FILES.iter().find(|(n, _)| *n == name)
    {
        return Some(Asset::text("text/plain; charset=utf-8", (*src).to_string()));
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
