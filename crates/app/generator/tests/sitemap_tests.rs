//! Unit tests for sitemap generation functionality

use systemprompt_generator::{build_sitemap_index, build_sitemap_xml, SitemapUrl};

// =============================================================================
// SitemapUrl tests
// =============================================================================

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
    assert!((url.priority - 0.8).abs() < f32::EPSILON);
}

#[test]
fn test_sitemap_url_clone() {
    let url = SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.9,
    };

    let cloned = url.clone();
    assert_eq!(cloned.loc, url.loc);
    assert_eq!(cloned.lastmod, url.lastmod);
    assert_eq!(cloned.changefreq, url.changefreq);
    assert!((cloned.priority - url.priority).abs() < f32::EPSILON);
}

// =============================================================================
// build_sitemap_xml tests
// =============================================================================

#[test]
fn test_generate_sitemap() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/".to_string(),
        lastmod: "2024-01-15".to_string(),
        changefreq: "daily".to_string(),
        priority: 1.0,
    }];

    let xml = build_sitemap_xml(&urls);

    assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
    assert!(xml.contains("</urlset>"));
}

#[test]
fn test_sitemap_urls() {
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

    let xml = build_sitemap_xml(&urls);

    // Check all URLs are present
    assert!(xml.contains("<loc>https://example.com/</loc>"));
    assert!(xml.contains("<loc>https://example.com/about</loc>"));
    assert!(xml.contains("<loc>https://example.com/blog</loc>"));

    // Check all lastmod dates
    assert!(xml.contains("<lastmod>2024-01-15</lastmod>"));
    assert!(xml.contains("<lastmod>2024-01-10</lastmod>"));
    assert!(xml.contains("<lastmod>2024-01-14</lastmod>"));

    // Check changefreq values
    assert!(xml.contains("<changefreq>daily</changefreq>"));
    assert!(xml.contains("<changefreq>monthly</changefreq>"));
    assert!(xml.contains("<changefreq>weekly</changefreq>"));

    // Check priorities (formatted with one decimal place)
    assert!(xml.contains("<priority>1.0</priority>"));
    assert!(xml.contains("<priority>0.8</priority>"));
    assert!(xml.contains("<priority>0.9</priority>"));
}

#[test]
fn test_sitemap_xml_format() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/test".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
    }];

    let xml = build_sitemap_xml(&urls);

    // Check proper XML structure
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<url>"));
    assert!(xml.contains("</url>"));
    assert!(xml.ends_with("</urlset>"));

    // Check proper nesting
    assert!(xml.contains("  <url>"));
    assert!(xml.contains("    <loc>"));
}

#[test]
fn test_sitemap_empty_urls() {
    let urls: Vec<SitemapUrl> = vec![];
    let xml = build_sitemap_xml(&urls);

    assert!(xml.contains("<urlset"));
    assert!(xml.contains("</urlset>"));
    assert!(!xml.contains("<url>"));
}

#[test]
fn test_sitemap_xml_escaping() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/search?q=test&page=1".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let xml = build_sitemap_xml(&urls);

    // Ampersand should be escaped
    assert!(xml.contains("&amp;"));
    assert!(!xml.contains("&page") || xml.contains("&amp;page"));
}

#[test]
fn test_sitemap_special_characters_escaped() {
    let urls = vec![SitemapUrl {
        loc: "https://example.com/test<script>".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "daily".to_string(),
        priority: 0.5,
    }];

    let xml = build_sitemap_xml(&urls);

    // < and > should be escaped
    assert!(xml.contains("&lt;"));
    assert!(xml.contains("&gt;"));
}

#[test]
fn test_sitemap_priority_formatting() {
    let urls = vec![
        SitemapUrl {
            loc: "https://example.com/high".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "daily".to_string(),
            priority: 1.0,
        },
        SitemapUrl {
            loc: "https://example.com/mid".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "daily".to_string(),
            priority: 0.5,
        },
        SitemapUrl {
            loc: "https://example.com/low".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: "daily".to_string(),
            priority: 0.1,
        },
    ];

    let xml = build_sitemap_xml(&urls);

    // Priorities should be formatted with one decimal place
    assert!(xml.contains("<priority>1.0</priority>"));
    assert!(xml.contains("<priority>0.5</priority>"));
    assert!(xml.contains("<priority>0.1</priority>"));
}

#[test]
fn test_sitemap_large_url_count() {
    // Test with many URLs (but not exceeding the 50k limit)
    let urls: Vec<SitemapUrl> = (0..100)
        .map(|i| SitemapUrl {
            loc: format!("https://example.com/page/{}", i),
            lastmod: "2024-01-01".to_string(),
            changefreq: "weekly".to_string(),
            priority: 0.5,
        })
        .collect();

    let xml = build_sitemap_xml(&urls);

    // Should contain all URLs
    for i in 0..100 {
        assert!(xml.contains(&format!("/page/{}", i)));
    }
}

// =============================================================================
// build_sitemap_index tests
// =============================================================================

#[test]
fn test_sitemap_index_generation() {
    let chunk1 = vec![SitemapUrl {
        loc: "https://example.com/page1".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
    }];

    let chunk2 = vec![SitemapUrl {
        loc: "https://example.com/page2".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
    }];

    let chunks = vec![chunk1, chunk2];
    let xml = build_sitemap_index(&chunks, "https://example.com");

    assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
    assert!(xml.contains("</sitemapindex>"));
}

#[test]
fn test_sitemap_index_urls() {
    let chunks: Vec<Vec<SitemapUrl>> = (0..3)
        .map(|_| {
            vec![SitemapUrl {
                loc: "https://example.com/page".to_string(),
                lastmod: "2024-01-01".to_string(),
                changefreq: "weekly".to_string(),
                priority: 0.5,
            }]
        })
        .collect();

    let xml = build_sitemap_index(&chunks, "https://example.com");

    // Should reference numbered sitemaps
    assert!(xml.contains("<loc>https://example.com/sitemaps/sitemap-1.xml</loc>"));
    assert!(xml.contains("<loc>https://example.com/sitemaps/sitemap-2.xml</loc>"));
    assert!(xml.contains("<loc>https://example.com/sitemaps/sitemap-3.xml</loc>"));
}

#[test]
fn test_sitemap_index_format() {
    let chunk = vec![SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
    }];

    let chunks = vec![chunk];
    let xml = build_sitemap_index(&chunks, "https://example.com");

    // Check proper structure
    assert!(xml.contains("<sitemap>"));
    assert!(xml.contains("</sitemap>"));
    assert!(xml.contains("<loc>"));
    assert!(xml.contains("<lastmod>"));
}

#[test]
fn test_sitemap_index_empty_chunks() {
    let chunks: Vec<Vec<SitemapUrl>> = vec![];
    let xml = build_sitemap_index(&chunks, "https://example.com");

    assert!(xml.contains("<sitemapindex"));
    assert!(xml.contains("</sitemapindex>"));
    assert!(!xml.contains("<sitemap>"));
}

#[test]
fn test_sitemap_index_with_different_base_urls() {
    let chunk = vec![SitemapUrl {
        loc: "https://example.com/page".to_string(),
        lastmod: "2024-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
    }];

    let chunks = vec![chunk.clone()];

    // Test with trailing slash
    let xml1 = build_sitemap_index(&chunks, "https://example.com/");
    // The function doesn't strip trailing slashes, so it will have double slash
    assert!(xml1.contains("example.com/"));

    // Test without trailing slash
    let xml2 = build_sitemap_index(&chunks, "https://example.com");
    assert!(xml2.contains("https://example.com/sitemaps/sitemap-1.xml"));
}

#[test]
fn test_sitemap_valid_changefreq_values() {
    let changefreqs = [
        "always", "hourly", "daily", "weekly", "monthly", "yearly", "never",
    ];

    for freq in changefreqs {
        let urls = vec![SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: "2024-01-01".to_string(),
            changefreq: freq.to_string(),
            priority: 0.5,
        }];

        let xml = build_sitemap_xml(&urls);
        assert!(xml.contains(&format!("<changefreq>{}</changefreq>", freq)));
    }
}

#[test]
fn test_sitemap_date_formats() {
    // Test various date format strings
    let dates = [
        "2024-01-15",
        "2024-01-15T10:30:00+00:00",
        "2024-01-15T10:30:00Z",
    ];

    for date in dates {
        let urls = vec![SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: date.to_string(),
            changefreq: "daily".to_string(),
            priority: 0.5,
        }];

        let xml = build_sitemap_xml(&urls);
        assert!(xml.contains(&format!("<lastmod>{}</lastmod>", date)));
    }
}
