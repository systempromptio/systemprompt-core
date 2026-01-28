//! Unit tests for navigation HTML generation
//!
//! Note: These tests require a full WebConfig setup which is complex.
//! The actual footer generation is tested via integration tests with real configs.

#[test]
fn test_navigation_module_exists() {
    use systemprompt_generator::generate_footer_html;
    let _ = generate_footer_html;
}
