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
    ("main", include_str!("../../web/css/main.css")),
];

const LIT_VENDOR: &str = include_str!("../../web/vendor/lit-all.min.js");

const ICON_SVG: &str = include_str!("../../assets/icon.svg");
const LOGO_SVG: &str = include_str!("../../assets/logo.svg");
const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.woff2");
const FONT_INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.woff2");
const FONT_OPENSANS_REGULAR: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Regular.woff2");
const FONT_OPENSANS_BOLD: &[u8] = include_bytes!("../../assets/fonts/OpenSans-Bold.woff2");

const JS_MODULES: &[(&str, &str)] = &[
    ("agents", include_str!("../../web/js/agents.js")),
    ("i18n", include_str!("../../web/js/i18n.js")),
    ("theme", include_str!("../../web/js/theme.js")),
    ("components/log-virtual", include_str!("../../web/js/components/log-virtual.js")),
    ("api", include_str!("../../web/js/api.js")),
    ("atoms", include_str!("../../web/js/atoms.js")),
    ("bridge", include_str!("../../web/js/bridge.js")),
    ("components/base", include_str!("../../web/js/components/base.js")),
    (
        "components/sp-cloud-status",
        include_str!("../../web/js/components/sp-cloud-status.js"),
    ),
    ("crumb", include_str!("../../web/js/crumb.js")),
    ("dom", include_str!("../../web/js/dom.js")),
    ("drawer", include_str!("../../web/js/drawer.js")),
    ("footer", include_str!("../../web/js/footer.js")),
    ("hosts", include_str!("../../web/js/hosts.js")),
    ("index", include_str!("../../web/js/index.js")),
    ("marketplace", include_str!("../../web/js/marketplace.js")),
    (
        "overall-badge",
        include_str!("../../web/js/overall-badge.js"),
    ),
    ("profile", include_str!("../../web/js/profile.js")),
    ("proxy", include_str!("../../web/js/proxy.js")),
    (
        "rail-indicator",
        include_str!("../../web/js/rail-indicator.js"),
    ),
    ("setup", include_str!("../../web/js/setup.js")),
    ("state", include_str!("../../web/js/state.js")),
    ("sync-pill", include_str!("../../web/js/sync-pill.js")),
    ("tabs", include_str!("../../web/js/tabs.js")),
    (
        "events/keyboard",
        include_str!("../../web/js/events/keyboard.js"),
    ),
    (
        "events/registry",
        include_str!("../../web/js/events/registry.js"),
    ),
    (
        "marketplace/detail",
        include_str!("../../web/js/marketplace/detail.js"),
    ),
    (
        "marketplace/glyph",
        include_str!("../../web/js/marketplace/glyph.js"),
    ),
    (
        "marketplace/list",
        include_str!("../../web/js/marketplace/list.js"),
    ),
    (
        "marketplace/state",
        include_str!("../../web/js/marketplace/state.js"),
    ),
    (
        "setup/gateway",
        include_str!("../../web/js/setup/gateway.js"),
    ),
    ("setup/agents", include_str!("../../web/js/setup/agents.js")),
    ("setup/mode", include_str!("../../web/js/setup/mode.js")),
    ("hosts/card", include_str!("../../web/js/hosts/card.js")),
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

pub fn render_index(token: &str) -> String {
    HTML.replace("__VERSION__", VERSION)
        .replace("__GIT_SHA__", git_sha_short())
        .replace("__BUILD_DATE__", BUILD_DATE)
        .replace("__ICON_SVG__", ICON_SVG)
        .replace("__LOGO_SVG__", LOGO_SVG)
        .replace("__PLATFORM_DISPLAY__", PLATFORM_DISPLAY)
        .replace("__PLATFORM__", PLATFORM_SLUG)
        .replace("__TOKEN__", token)
}

pub fn lookup_path(path: &str, token: &str) -> Option<Asset> {
    if path == "/" || path == "/index.html" {
        return Some(Asset::text(
            "text/html; charset=utf-8",
            render_index(token),
        ));
    }
    if let Some(name) = path
        .strip_prefix("/assets/css/")
        .and_then(|s| s.strip_suffix(".css"))
    {
        if let Some((_, src)) = CSS_FILES.iter().find(|(n, _)| *n == name) {
            return Some(Asset::text(
                "text/css; charset=utf-8",
                src.replace("__TOKEN__", token),
            ));
        }
    }
    if path == "/assets/js/vendor/lit-all.js" {
        return Some(Asset::text(
            "application/javascript; charset=utf-8",
            LIT_VENDOR.to_string(),
        ));
    }
    if let Some(name) = path
        .strip_prefix("/assets/js/")
        .and_then(|s| s.strip_suffix(".js"))
    {
        if let Some((_, src)) = JS_MODULES.iter().find(|(n, _)| *n == name) {
            return Some(Asset::text(
                "application/javascript; charset=utf-8",
                src.replace("__TOKEN__", token),
            ));
        }
    }
    if let Some(name) = path
        .strip_prefix("/assets/i18n/")
        .and_then(|s| s.strip_suffix(".ftl"))
    {
        if let Some((_, src)) = I18N_FILES.iter().find(|(n, _)| *n == name) {
            return Some(Asset::text(
                "text/plain; charset=utf-8",
                (*src).to_string(),
            ));
        }
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
