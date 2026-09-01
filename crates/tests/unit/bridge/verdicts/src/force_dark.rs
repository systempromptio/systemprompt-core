//! `Brand::force_dark` has to reach the page, not just the window: the GUI
//! resolves its theme in `theme.js`, which only knows about the flag through
//! the `__SP_FORCE_DARK__` marker `render_index_from` injects. This proves the
//! marker is emitted for a brand that sets the flag, and withheld for one that
//! does not — the whole mechanism is that one string.

use systemprompt_bridge::brand::{Brand, set_brand};
use systemprompt_bridge::web_assets::render_index_from;

static DARK: Brand = Brand {
    force_dark: true,
    ..Brand::SYSTEMPROMPT
};

const SHELL: &str = "<html><head><title>t</title></head><body></body></html>";
const MARKER: &str = "window.__SP_FORCE_DARK__ = true;";

#[test]
fn force_dark_brand_injects_the_marker_before_head_closes() {
    assert!(
        !render_index_from(SHELL).contains(MARKER),
        "the default brand must not pin the theme"
    );

    assert!(set_brand(&DARK), "brand already set");
    let html = render_index_from(SHELL);
    assert!(
        html.contains(MARKER),
        "force_dark brand did not inject the marker"
    );
    assert!(
        html.find(MARKER) < html.find("</head>"),
        "marker must land inside <head>, before the module scripts"
    );
}
