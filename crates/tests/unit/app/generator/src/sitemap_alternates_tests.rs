use systemprompt_generator::{SitemapUrl, build_sitemap_xml, escape_xml};

#[test]
fn sitemap_url_alternates_field_empty_by_default() {
    let url = SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "monthly".to_string(),
        priority: 0.7,
        alternates: vec![],
    };
    assert!(url.alternates.is_empty());
}

#[test]
fn sitemap_url_debug_format() {
    let url = SitemapUrl {
        loc: "https://example.com/test".to_string(),
        lastmod: "2025-01-15".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.8,
        alternates: vec![],
    };
    let dbg = format!("{:?}", url);
    assert!(dbg.contains("SitemapUrl"));
    assert!(dbg.contains("example.com"));
    assert!(dbg.contains("weekly"));
}

#[test]
fn sitemap_url_clone_preserves_alternates_empty() {
    let url = SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
        alternates: vec![],
    };
    let cloned = url.clone();
    assert_eq!(cloned.loc, url.loc);
    assert!(cloned.alternates.is_empty());
}

#[test]
fn build_sitemap_xml_no_xhtml_link_when_no_alternates() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "monthly".to_string(),
        priority: 0.5,
        alternates: vec![],
    }];
    let xml = build_sitemap_xml(&urls);
    assert!(
        !xml.contains("xhtml:link"),
        "no alternates = no xhtml:link elements"
    );
}

#[test]
fn build_sitemap_xml_xhtml_namespace_always_declared() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
        alternates: vec![],
    }];
    let xml = build_sitemap_xml(&urls);
    assert!(xml.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""));
}

#[test]
fn sitemap_url_priority_boundary_values() {
    for priority in [0.0_f32, 0.1, 0.5, 0.9, 1.0] {
        let urls = vec![SitemapUrl {
            loc: format!("https://example.com/p{}", (priority * 10.0) as u32),
            lastmod: "2025-01-01".to_string(),
            changefreq: "monthly".to_string(),
            priority,
            alternates: vec![],
        }];
        let xml = build_sitemap_xml(&urls);
        assert!(
            xml.contains("<priority>"),
            "priority field should appear for {priority}"
        );
    }
}

#[test]
fn build_sitemap_xml_with_many_entries_preserves_order() {
    let urls: Vec<SitemapUrl> = (0..5)
        .map(|i| SitemapUrl {
            loc: format!("https://example.com/item/{i}"),
            lastmod: "2025-01-01".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.5,
            alternates: vec![],
        })
        .collect();
    let xml = build_sitemap_xml(&urls);
    let pos0 = xml.find("/item/0").unwrap();
    let pos4 = xml.find("/item/4").unwrap();
    assert!(pos0 < pos4, "entries should appear in input order");
}

#[test]
fn escape_xml_apostrophe_escaping() {
    assert_eq!(escape_xml("it's"), "it&apos;s");
}

#[test]
fn escape_xml_ampersand_escaping_repeated() {
    assert_eq!(escape_xml("&&"), "&amp;&amp;");
}

#[test]
fn escape_xml_less_than_greater_than() {
    assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
}

#[test]
fn escape_xml_quote() {
    assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
}

#[test]
fn escape_xml_all_five_specials() {
    let raw = "& < > \" '";
    let esc = escape_xml(raw);
    assert_eq!(esc, "&amp; &lt; &gt; &quot; &apos;");
}

#[test]
fn sitemap_url_with_encoded_path_chars() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/path%20with%20spaces".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
        alternates: vec![],
    }];
    let xml = build_sitemap_xml(&urls);
    assert!(xml.contains("path%20with%20spaces"));
}

#[test]
fn sitemap_xml_starts_with_xml_declaration() {
    let xml = build_sitemap_xml(&[]);
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
}

#[test]
fn sitemap_xml_ends_with_closing_urlset() {
    let xml = build_sitemap_xml(&[]);
    assert!(xml.ends_with("</urlset>"));
}

#[test]
fn sitemap_index_multiple_chunks_correctly_numbered() {
    use systemprompt_generator::build_sitemap_index;
    let chunks: Vec<Vec<SitemapUrl>> = (0..5)
        .map(|_| {
            vec![SitemapUrl {
                loc: "https://example.com/x".to_string(),
                lastmod: "2025-01-01".to_string(),
                changefreq: "weekly".to_string(),
                priority: 0.5,
                alternates: vec![],
            }]
        })
        .collect();
    let xml = build_sitemap_index(&chunks, "https://example.com");
    assert!(xml.contains("sitemap-1.xml"));
    assert!(xml.contains("sitemap-2.xml"));
    assert!(xml.contains("sitemap-3.xml"));
    assert!(xml.contains("sitemap-4.xml"));
    assert!(xml.contains("sitemap-5.xml"));
}

#[test]
fn sitemap_index_base_url_used_in_loc() {
    use systemprompt_generator::build_sitemap_index;
    let chunks = vec![vec![SitemapUrl {
        loc: "https://example.com/a".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
        alternates: vec![],
    }]];
    let xml = build_sitemap_index(&chunks, "https://my.site.com");
    assert!(xml.contains("https://my.site.com/sitemaps/sitemap-1.xml"));
}
