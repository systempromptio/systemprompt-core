use std::path::Path;

use systemprompt_bridge::gui::command::removal_method;

#[test]
fn classifies_supported_application_removal_routes() {
    assert_eq!(
        removal_method(
            Path::new(r"C:\Users\person\scoop\apps\bridge\current\systemprompt-bridge.exe"),
            "windows",
        ),
        "scoop"
    );
    assert_eq!(
        removal_method(Path::new("/Applications/systemprompt bridge.app"), "macos"),
        "macos"
    );
    assert_eq!(
        removal_method(Path::new(r"C:\Tools\systemprompt-bridge.exe"), "windows"),
        "standalone"
    );
}
