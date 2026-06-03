use systemprompt_bridge::gui::error::GuiError;
use systemprompt_bridge::gui::handlers::profile::is_logged_out_error;

#[test]
fn not_authenticated_is_treated_as_logged_out() {
    assert!(is_logged_out_error(&GuiError::NotAuthenticated));
}

#[test]
fn other_errors_are_surfaced() {
    assert!(!is_logged_out_error(&GuiError::Io(std::io::Error::other(
        "boom"
    ))));
}
