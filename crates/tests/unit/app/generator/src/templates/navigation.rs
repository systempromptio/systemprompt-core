//! Unit tests for navigation HTML generation

use systemprompt_generator::generate_footer_html;

fn make_web_config(yaml_str: &str) -> serde_yaml::Value {
    serde_yaml::from_str(yaml_str).expect("Failed to parse YAML")
}

// ============================================================================
// generate_footer_html Tests
// ============================================================================

#[test]
fn test_generate_footer_html_minimal_config() {
    let config = make_web_config(
        r#"
branding:
  copyright: "© 2024 Test Company"
navigation:
  footer: {}
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("© 2024 Test Company"));
    assert!(html.contains("footer-meta"));
}

#[test]
fn test_generate_footer_html_with_single_section() {
    let config = make_web_config(
        r#"
branding:
  copyright: "© 2024 Test"
navigation:
  footer:
    resources:
      - path: "/docs"
        label: "Documentation"
      - path: "/api"
        label: "API Reference"
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("footer-nav"));
    assert!(html.contains("Resources"));
    assert!(html.contains("href=\"/docs\""));
    assert!(html.contains("Documentation"));
    assert!(html.contains("href=\"/api\""));
    assert!(html.contains("API Reference"));
}

#[test]
fn test_generate_footer_html_with_multiple_sections() {
    let config = make_web_config(
        r#"
branding:
  copyright: "© 2024"
navigation:
  footer:
    resources:
      - path: "/docs"
        label: "Documentation"
    company:
      - path: "/about"
        label: "About Us"
      - path: "/contact"
        label: "Contact"
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("Resources"));
    assert!(html.contains("Company"));
    assert!(html.contains("/docs"));
    assert!(html.contains("/about"));
    assert!(html.contains("/contact"));
}

#[test]
fn test_generate_footer_html_with_social_links() {
    let config = make_web_config(
        r#"
branding:
  copyright: "© 2024"
navigation:
  footer: {}
  social:
    - type: "github"
      href: "https://github.com/test"
      label: "GitHub"
    - type: "twitter"
      href: "https://twitter.com/test"
      label: "Twitter"
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("footer-social"));
    assert!(html.contains("https://github.com/test"));
    assert!(html.contains("GitHub"));
}

#[test]
fn test_generate_footer_html_missing_copyright_fails() {
    let config = make_web_config(
        r#"
navigation:
  footer: {}
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_err());
}

#[test]
fn test_generate_footer_html_empty_footer_section() {
    let config = make_web_config(
        r#"
branding:
  copyright: "Test"
navigation:
  footer:
    resources: []
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(!html.contains("Resources"));
}

#[test]
fn test_generate_footer_html_social_link_types() {
    let config = make_web_config(
        r#"
branding:
  copyright: "Test"
navigation:
  footer: {}
  social:
    - type: "github"
      href: "https://github.com"
      label: "GitHub"
    - type: "twitter"
      href: "https://twitter.com"
      label: "Twitter"
    - type: "linkedin"
      href: "https://linkedin.com"
      label: "LinkedIn"
    - type: "email"
      href: "mailto:test@example.com"
      label: "Email"
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("<svg"));
}

#[test]
fn test_generate_footer_html_unknown_social_type() {
    let config = make_web_config(
        r#"
branding:
  copyright: "Test"
navigation:
  footer: {}
  social:
    - type: "unknown"
      href: "https://unknown.com"
      label: "Unknown"
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());
}

#[test]
fn test_generate_footer_html_copyright_with_special_chars() {
    let config = make_web_config(
        r#"
branding:
  copyright: "© 2024 Test & Company <Inc>"
navigation:
  footer: {}
"#,
    );

    let result = generate_footer_html(&config);
    assert!(result.is_ok());

    let html = result.unwrap();
    assert!(html.contains("© 2024 Test & Company <Inc>"));
}
