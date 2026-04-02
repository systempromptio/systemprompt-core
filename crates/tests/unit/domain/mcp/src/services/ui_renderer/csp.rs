use systemprompt_mcp::services::ui_renderer::{CspBuilder, CspPolicy};

#[test]
fn strict_policy_default_src_contains_self() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.default_src, vec!["'self'"]);
}

#[test]
fn strict_policy_script_src_contains_self_and_unsafe_inline() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.script_src, vec!["'self'", "'unsafe-inline'"]);
}

#[test]
fn strict_policy_style_src_contains_self_and_unsafe_inline() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.style_src, vec!["'self'", "'unsafe-inline'"]);
}

#[test]
fn strict_policy_img_src_contains_self_and_data() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.img_src, vec!["'self'", "data:"]);
}

#[test]
fn strict_policy_connect_src_contains_self() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.connect_src, vec!["'self'"]);
}

#[test]
fn strict_policy_font_src_contains_self() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.font_src, vec!["'self'"]);
}

#[test]
fn strict_policy_frame_src_is_none() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.frame_src, vec!["'none'"]);
}

#[test]
fn strict_policy_base_uri_contains_self() {
    let policy = CspPolicy::strict();
    assert_eq!(policy.base_uri, vec!["'self'"]);
}

#[test]
fn to_header_value_contains_all_directives() {
    let policy = CspPolicy::strict();
    let header = policy.to_header_value();

    assert!(header.contains("default-src 'self'"));
    assert!(header.contains("script-src 'self' 'unsafe-inline'"));
    assert!(header.contains("style-src 'self' 'unsafe-inline'"));
    assert!(header.contains("img-src 'self' data:"));
    assert!(header.contains("connect-src 'self'"));
    assert!(header.contains("font-src 'self'"));
    assert!(header.contains("frame-src 'none'"));
    assert!(header.contains("base-uri 'self'"));
}

#[test]
fn to_header_value_directives_separated_by_semicolons() {
    let policy = CspPolicy::strict();
    let header = policy.to_header_value();
    let parts: Vec<&str> = header.split("; ").collect();
    assert_eq!(parts.len(), 8);
}

#[test]
fn to_header_value_empty_directive_omitted() {
    let mut policy = CspPolicy::strict();
    policy.font_src.clear();
    let header = policy.to_header_value();
    assert!(!header.contains("font-src"));
}

#[test]
fn to_header_value_all_empty_produces_empty_string() {
    let policy = CspPolicy::default();
    let header = policy.to_header_value();
    assert!(header.is_empty());
}

#[test]
fn with_cdn_single_origin() {
    let policy = CspPolicy::with_cdn(&["https://cdn.example.com"]);
    assert!(policy.script_src.contains(&"https://cdn.example.com".to_string()));
    assert!(policy.style_src.contains(&"https://cdn.example.com".to_string()));
}

#[test]
fn with_cdn_multiple_origins() {
    let policy = CspPolicy::with_cdn(&["https://cdn1.example.com", "https://cdn2.example.com"]);
    assert!(policy.script_src.contains(&"https://cdn1.example.com".to_string()));
    assert!(policy.script_src.contains(&"https://cdn2.example.com".to_string()));
    assert!(policy.style_src.contains(&"https://cdn1.example.com".to_string()));
    assert!(policy.style_src.contains(&"https://cdn2.example.com".to_string()));
}

#[test]
fn with_cdn_preserves_strict_base() {
    let policy = CspPolicy::with_cdn(&["https://cdn.example.com"]);
    assert!(policy.script_src.contains(&"'self'".to_string()));
    assert!(policy.script_src.contains(&"'unsafe-inline'".to_string()));
    assert_eq!(policy.default_src, vec!["'self'"]);
    assert_eq!(policy.frame_src, vec!["'none'"]);
}

#[test]
fn with_cdn_empty_origins() {
    let strict = CspPolicy::strict();
    let policy = CspPolicy::with_cdn(&[]);
    assert_eq!(policy.script_src, strict.script_src);
    assert_eq!(policy.style_src, strict.style_src);
}

#[test]
fn with_cdn_does_not_modify_connect_src() {
    let policy = CspPolicy::with_cdn(&["https://cdn.example.com"]);
    assert_eq!(policy.connect_src, vec!["'self'"]);
}

#[test]
fn builder_new_creates_empty_policy() {
    let policy = CspBuilder::new().build();
    assert!(policy.default_src.is_empty());
    assert!(policy.script_src.is_empty());
}

#[test]
fn builder_strict_starts_from_strict_policy() {
    let policy = CspBuilder::strict().build();
    let strict = CspPolicy::strict();
    assert_eq!(policy.default_src, strict.default_src);
    assert_eq!(policy.script_src, strict.script_src);
}

#[test]
fn builder_default_src() {
    let policy = CspBuilder::new()
        .default_src(vec!["'none'".to_string()])
        .build();
    assert_eq!(policy.default_src, vec!["'none'"]);
}

#[test]
fn builder_script_src_replaces() {
    let policy = CspBuilder::strict()
        .script_src(vec!["'self'".to_string()])
        .build();
    assert_eq!(policy.script_src, vec!["'self'"]);
}

#[test]
fn builder_add_script_src_appends() {
    let policy = CspBuilder::strict()
        .add_script_src("https://example.com")
        .build();
    assert_eq!(policy.script_src.len(), 3);
    assert!(policy.script_src.contains(&"https://example.com".to_string()));
}

#[test]
fn builder_style_src_replaces() {
    let policy = CspBuilder::new()
        .style_src(vec!["'self'".to_string()])
        .build();
    assert_eq!(policy.style_src, vec!["'self'"]);
}

#[test]
fn builder_add_style_src_appends() {
    let policy = CspBuilder::strict()
        .add_style_src("https://fonts.googleapis.com")
        .build();
    assert!(policy.style_src.contains(&"https://fonts.googleapis.com".to_string()));
}

#[test]
fn builder_img_src() {
    let policy = CspBuilder::new()
        .img_src(vec!["'self'".to_string(), "https:".to_string()])
        .build();
    assert_eq!(policy.img_src, vec!["'self'", "https:"]);
}

#[test]
fn builder_connect_src() {
    let policy = CspBuilder::new()
        .connect_src(vec!["'self'".to_string()])
        .build();
    assert_eq!(policy.connect_src, vec!["'self'"]);
}

#[test]
fn builder_add_connect_src_appends() {
    let policy = CspBuilder::strict()
        .add_connect_src("wss://api.example.com")
        .build();
    assert!(policy.connect_src.contains(&"wss://api.example.com".to_string()));
}

#[test]
fn builder_font_src() {
    let policy = CspBuilder::new()
        .font_src(vec!["https://fonts.gstatic.com".to_string()])
        .build();
    assert_eq!(policy.font_src, vec!["https://fonts.gstatic.com"]);
}

#[test]
fn builder_frame_src() {
    let policy = CspBuilder::new()
        .frame_src(vec!["'none'".to_string()])
        .build();
    assert_eq!(policy.frame_src, vec!["'none'"]);
}

#[test]
fn builder_base_uri() {
    let policy = CspBuilder::new()
        .base_uri(vec!["'self'".to_string()])
        .build();
    assert_eq!(policy.base_uri, vec!["'self'"]);
}

#[test]
fn builder_chaining_multiple_directives() {
    let policy = CspBuilder::new()
        .default_src(vec!["'self'".to_string()])
        .script_src(vec!["'self'".to_string()])
        .style_src(vec!["'self'".to_string()])
        .img_src(vec!["'self'".to_string()])
        .connect_src(vec!["'self'".to_string()])
        .font_src(vec!["'self'".to_string()])
        .frame_src(vec!["'none'".to_string()])
        .base_uri(vec!["'self'".to_string()])
        .build();

    let header = policy.to_header_value();
    assert!(header.contains("default-src 'self'"));
    assert!(header.contains("frame-src 'none'"));
}

#[test]
fn builder_add_multiple_script_sources() {
    let policy = CspBuilder::strict()
        .add_script_src("https://cdn1.example.com")
        .add_script_src("https://cdn2.example.com")
        .build();
    assert_eq!(policy.script_src.len(), 4);
}

#[test]
fn to_mcp_domains_filters_out_quoted_sources() {
    let policy = CspPolicy::strict();
    let domains = policy.to_mcp_domains();
    assert!(domains.connect.is_empty());
}

#[test]
fn to_mcp_domains_includes_real_domains() {
    let mut policy = CspPolicy::strict();
    policy.connect_src.push("https://api.example.com".to_string());
    let domains = policy.to_mcp_domains();
    assert_eq!(domains.connect, vec!["https://api.example.com"]);
}

#[test]
fn to_mcp_domains_filters_data_scheme() {
    let policy = CspPolicy::strict();
    let domains = policy.to_mcp_domains();
    assert!(domains.resources.is_empty());
}

#[test]
fn to_mcp_domains_resource_domains_deduped() {
    let mut policy = CspPolicy::strict();
    policy.script_src.push("https://cdn.example.com".to_string());
    policy.style_src.push("https://cdn.example.com".to_string());
    let domains = policy.to_mcp_domains();
    let cdn_count = domains
        .resources
        .iter()
        .filter(|d| *d == "https://cdn.example.com")
        .count();
    assert_eq!(cdn_count, 1);
}

#[test]
fn to_mcp_domains_resource_domains_sorted() {
    let mut policy = CspPolicy::strict();
    policy.script_src.push("https://z-cdn.example.com".to_string());
    policy.script_src.push("https://a-cdn.example.com".to_string());
    let domains = policy.to_mcp_domains();
    let z_pos = domains.resources.iter().position(|d| d == "https://z-cdn.example.com");
    let a_pos = domains.resources.iter().position(|d| d == "https://a-cdn.example.com");
    assert!(a_pos.unwrap() < z_pos.unwrap());
}

#[test]
fn to_mcp_domains_frames_filtered() {
    let policy = CspPolicy::strict();
    let domains = policy.to_mcp_domains();
    assert!(domains.frames.is_empty());
}

#[test]
fn default_policy_all_fields_empty() {
    let policy = CspPolicy::default();
    assert!(policy.default_src.is_empty());
    assert!(policy.script_src.is_empty());
    assert!(policy.style_src.is_empty());
    assert!(policy.img_src.is_empty());
    assert!(policy.connect_src.is_empty());
    assert!(policy.font_src.is_empty());
    assert!(policy.frame_src.is_empty());
    assert!(policy.base_uri.is_empty());
}

#[test]
fn policy_serializes_to_json() {
    let policy = CspPolicy::strict();
    let json = serde_json::to_value(&policy).unwrap();
    assert!(json.get("default_src").is_some());
    assert!(json.get("script_src").is_some());
}

#[test]
fn policy_deserializes_from_json() {
    let json = serde_json::json!({
        "default_src": ["'self'"],
        "script_src": ["'self'"],
        "style_src": [],
        "img_src": [],
        "connect_src": [],
        "font_src": [],
        "frame_src": [],
        "base_uri": []
    });
    let policy: CspPolicy = serde_json::from_value(json).unwrap();
    assert_eq!(policy.default_src, vec!["'self'"]);
}

#[test]
fn policy_roundtrip_serialization() {
    let original = CspPolicy::strict();
    let json = serde_json::to_string(&original).unwrap();
    let restored: CspPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(original.to_header_value(), restored.to_header_value());
}

#[test]
fn header_value_single_directive() {
    let policy = CspBuilder::new()
        .default_src(vec!["'self'".to_string()])
        .build();
    let header = policy.to_header_value();
    assert_eq!(header, "default-src 'self'");
}

#[test]
fn header_value_multiple_sources_space_separated() {
    let policy = CspBuilder::new()
        .script_src(vec![
            "'self'".to_string(),
            "'unsafe-inline'".to_string(),
            "https://cdn.example.com".to_string(),
        ])
        .build();
    let header = policy.to_header_value();
    assert_eq!(header, "script-src 'self' 'unsafe-inline' https://cdn.example.com");
}
