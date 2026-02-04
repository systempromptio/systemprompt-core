use systemprompt_mcp::services::ui_renderer::{CspBuilder, CspPolicy};

#[test]
fn test_strict_policy() {
    let policy = CspPolicy::strict();
    let header = policy.to_header_value();

    assert!(header.contains("default-src 'self'"));
    assert!(header.contains("script-src 'self' 'unsafe-inline'"));
    assert!(header.contains("frame-src 'none'"));
}

#[test]
fn test_cdn_policy() {
    let policy = CspPolicy::with_cdn(&["https://cdn.jsdelivr.net"]);
    let header = policy.to_header_value();

    assert!(header.contains("https://cdn.jsdelivr.net"));
}

#[test]
fn test_builder() {
    let policy = CspBuilder::strict()
        .add_script_src("https://example.com")
        .add_connect_src("wss://api.example.com")
        .build();

    let header = policy.to_header_value();
    assert!(header.contains("https://example.com"));
    assert!(header.contains("wss://api.example.com"));
}
