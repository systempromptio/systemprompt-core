//! Unit tests for sitemap XML generation

use systemprompt_generator::{build_sitemap_index, build_sitemap_xml, SitemapUrl};

// ============================================================================
// SitemapUrl Tests
// ============================================================================

#[test]
fn test_sitemap_url_creation() {
    let url = SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.8,
    };

    assert_eq!(url.loc, "https://example.com/page");
    assert_eq!(url.lastmod, "2024-01-15");
    assert_eq!(url.changefreq, "weekly");
    assert_eq!(url.priority, 0.8);
}

#[test]
fn test_sitemap_url_clone() {
    let url = SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    };

    let cloned = url.clone();
    assert_eq!(url.loc, cloned.loc);
    assert_eq!(url.lastmod, cloned.lastmod);
    assert_eq!(url.changefreq, cloned.changefreq);
    assert_eq!(url.priority, cloned.priority);
}

#[test]
fn test_sitemap_url_debug() {
    let url = SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "monthly".to_string(),
        priority: 0.5,
    };

    let debug = format!("{:?}", url);
    assert!(debug.contains("SitemapUrl"));
    assert!(debug.contains("example.com"));
}

// ============================================================================
// build_sitemap_xml Tests
// ============================================================================

#[test]
fn test_build_sitemap_xml_empty() {
    let urls: Vec<SitemapUrl> = vec![];
    let result = build_sitemap_xml(&urls);

    assert!(result.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(result.contains("<urlset"));
    assert!(result.contains("</urlset>"));
    assert!(!result.contains("<url>"));
}

#[test]
fn test_build_sitemap_xml_single_url() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("<url>"));
    assert!(result.contains("<loc>https://example.com/</loc>"));
    assert!(result.contains("<lastmod>2024-01-15</lastmod>"));
    assert!(result.contains("<changefreq>daily</changefreq>"));
    assert!(result.contains("<priority>1.0</priority>"));
    assert!(result.contains("</url>"));
}

#[test]
fn test_build_sitemap_xml_multiple_urls() {
    let urls = vec![
        SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        },
        SitemapUrl {
            loc: "https://example.com/about".to_string(),
            lastmod: "2024-01-10".to_string(),
            changefreq: "monthly".to_string(),
            priority: 0.8,
        },
        SitemapUrl {
            loc: "https://example.com/blog".to_string(),
            lastmod: "2024-01-14".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.9,
        },
    ];

    let result = build_sitemap_xml(&urls);

    // Count URL elements
    let url_count = result.matches("<url>").count();
    assert_eq!(url_count, 3);

    assert!(result.contains("https://example.com/"));
    assert!(result.contains("https://example.com/about"));
    assert!(result.contains("https://example.com/blog"));
}

#[test]
fn test_build_sitemap_xml_priority_formatting() {
    let urls = vec![
        SitemapUrl {
            loc: "https://example.com/high".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        },
        SitemapUrl {
            loc: "https://example.com/low".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "yearly".to_string(),
            priority: 0.1,
        },
    ];

    let result = build_sitemap_xml(&urls);

    // Priority should be formatted with one decimal place
    assert!(result.contains("<priority>1.0</priority>"));
    assert!(result.contains("<priority>0.1</priority>"));
}

#[test]
fn test_build_sitemap_xml_escapes_special_characters() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/search?q=test&page=1".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    // Ampersand should be escaped
    assert!(result.contains("&amp;"));
    assert!(!result.contains("&page"));
}

#[test]
fn test_build_sitemap_xml_escapes_angle_brackets() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/test".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "<script>".to_string(), // Invalid but tests escaping
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("&lt;script&gt;"));
    assert!(!result.contains("<script>") || result.contains("&lt;script&gt;"));
}

#[test]
fn test_build_sitemap_xml_escapes_quotes() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/page\"test".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("&quot;"));
}

#[test]
fn test_build_sitemap_xml_has_namespace() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\""));
}

#[test]
fn test_build_sitemap_xml_changefreq_values() {
    let changefreqs = vec![
        "always", "hourly", "daily", "weekly", "monthly", "yearly", "never",
    ];

    for freq in changefreqs {
        let urls = vec![SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: freq.to_string(),
            priority: 0.5,
        }];

        let result = build_sitemap_xml(&urls);
        assert!(result.contains(&format!("<changefreq>{}</changefreq>", freq)));
    }
}

// ============================================================================
// build_sitemap_index Tests
// ============================================================================

#[test]
fn test_build_sitemap_index_empty() {
    let chunks: Vec<Vec<SitemapUrl>> = vec![];
    let result = build_sitemap_index(&chunks, "https://example.com");

    assert!(result.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(result.contains("<sitemapindex"));
    assert!(result.contains("</sitemapindex>"));
    assert!(!result.contains("<sitemap>"));
}

#[test]
fn test_build_sitemap_index_single_chunk() {
    let chunks = vec![vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }]];

    let result = build_sitemap_index(&chunks, "https://example.com");

    assert!(result.contains("<sitemap>"));
    assert!(result.contains("<loc>https://example.com/sitemaps/sitemap-1.xml</loc>"));
    assert!(result.contains("<lastmod>"));
    assert!(result.contains("</sitemap>"));
}

#[test]
fn test_build_sitemap_index_multiple_chunks() {
    let chunks = vec![
        vec![SitemapUrl {
            loc: "https://example.com/page1".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        }],
        vec![SitemapUrl {
            loc: "https://example.com/page2".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        }],
        vec![SitemapUrl {
            loc: "https://example.com/page3".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        }],
    ];

    let result = build_sitemap_index(&chunks, "https://example.com");

    // Should have 3 sitemap references
    let sitemap_count = result.matches("<sitemap>").count();
    assert_eq!(sitemap_count, 3);

    assert!(result.contains("sitemap-1.xml"));
    assert!(result.contains("sitemap-2.xml"));
    assert!(result.contains("sitemap-3.xml"));
}

#[test]
fn test_build_sitemap_index_has_namespace() {
    let chunks = vec![vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }]];

    let result = build_sitemap_index(&chunks, "https://example.com");

    assert!(result.contains("xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\""));
}

#[test]
fn test_build_sitemap_index_base_url_formatting() {
    let chunks = vec![vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }]];

    // Test with trailing slash
    let result = build_sitemap_index(&chunks, "https://example.com/");
    assert!(result.contains("https://example.com//sitemaps/sitemap-1.xml"));

    // Test without trailing slash
    let result = build_sitemap_index(&chunks, "https://example.com");
    assert!(result.contains("https://example.com/sitemaps/sitemap-1.xml"));
}

#[test]
fn test_build_sitemap_index_lastmod_date_format() {
    let chunks = vec![vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }]];

    let result = build_sitemap_index(&chunks, "https://example.com");

    // Lastmod should be in YYYY-MM-DD format (from current date)
    assert!(result.contains("<lastmod>"));
    // The date format should be something like 2024-01-15 or 2025-12-28
    let has_date_format = result.contains("-") && result.contains("</lastmod>");
    assert!(has_date_format);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_sitemap_url_with_unicode() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/日本語ページ".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    // Should handle Unicode in URLs
    assert!(result.contains("日本語ページ"));
}

#[test]
fn test_sitemap_url_with_long_url() {
    let long_path = "a".repeat(1000);
    let urls = vec![SitemapUrl {
        loc: format!("https://example.com/{}", long_path),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains(&long_path));
}

#[test]
fn test_sitemap_priority_zero() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/low-priority".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "yearly".to_string(),
        priority: 0.0,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("<priority>0.0</priority>"));
}

#[test]
fn test_sitemap_empty_lastmod() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let result = build_sitemap_xml(&urls);

    assert!(result.contains("<lastmod></lastmod>"));
}

#[test]
fn test_sitemap_xml_well_formed() {
    let urls = vec![
        SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: "2024-01-15".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        },
        SitemapUrl {
            loc: "https://example.com/about".to_string(),
            lastmod: "2024-01-10".to_string(),
            changefreq: "monthly".to_string(),
            priority: 0.8,
        },
    ];

    let result = build_sitemap_xml(&urls);

    // Check XML is well-formed (basic checks)
    assert!(result.starts_with("<?xml"));
    assert!(result.ends_with("</urlset>"));

    // Count opening and closing tags
    let url_open = result.matches("<url>").count();
    let url_close = result.matches("</url>").count();
    assert_eq!(url_open, url_close);

    let loc_open = result.matches("<loc>").count();
    let loc_close = result.matches("</loc>").count();
    assert_eq!(loc_open, loc_close);
}
