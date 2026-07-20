use systemprompt_bridge::gui::assets::{lookup_path, render_index};

#[test]
fn render_index_substitutes_template_markers() {
    let html = render_index();
    assert!(!html.is_empty());
    assert!(
        !html.contains("__PLATFORM__"),
        "platform placeholder should be substituted"
    );
    assert!(
        !html.contains("__VERSION__"),
        "version placeholder should be substituted"
    );
}

#[test]
fn lookup_root_returns_index_html() {
    let asset = lookup_path("/").expect("root should resolve");
    assert_eq!(asset.content_type, "text/html; charset=utf-8");
    assert!(!asset.body.is_empty());

    let index = lookup_path("/index.html").expect("index.html should resolve");
    assert_eq!(index.content_type, "text/html; charset=utf-8");
}

#[test]
fn lookup_known_css_module() {
    let asset = lookup_path("/assets/css/tokens.css").expect("tokens.css should resolve");
    assert_eq!(asset.content_type, "text/css; charset=utf-8");
    assert!(!asset.body.is_empty());
}

#[test]
fn lookup_known_js_module() {
    let asset = lookup_path("/assets/js/index.js").expect("index.js should resolve");
    assert_eq!(asset.content_type, "application/javascript; charset=utf-8");
    assert!(!asset.body.is_empty());
}

#[test]
fn brand_overrides_sheet_is_served() {
    let asset =
        lookup_path("/assets/css/brand-overrides.css").expect("brand-overrides.css should resolve");
    assert_eq!(asset.content_type, "text/css; charset=utf-8");
}

#[test]
fn main_css_imports_brand_overrides_last() {
    let asset = lookup_path("/assets/css/main.css").expect("main.css should resolve");
    let body = std::str::from_utf8(&asset.body).expect("main.css is utf-8");
    let last_import = body
        .lines()
        .filter(|l| l.contains("@import"))
        .next_back()
        .expect("main.css has @import lines");
    assert!(
        last_import.contains("brand-overrides.css"),
        "brand-overrides must be the last import so brand rules win the cascade, got: {last_import}"
    );
}

#[test]
fn lookup_unknown_path_is_none() {
    assert!(lookup_path("/assets/css/does-not-exist.css").is_none());
    assert!(lookup_path("/totally/unknown").is_none());
}
