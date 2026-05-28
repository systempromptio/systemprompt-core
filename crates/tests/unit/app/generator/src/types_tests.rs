//! Tests for public re-exported value types: `PagePrerenderResult`,
//! `GeneratedFeed`, `SitemapUrl` accessors, and various Debug impls.

use std::path::PathBuf;
use systemprompt_generator::{
    GeneratedFeed, PagePrerenderResult, RssChannel, SitemapUrl, build_rss_xml, build_sitemap_index,
    build_sitemap_xml, escape_xml,
};

#[test]
fn page_prerender_result_debug_contains_page_type() {
    let r = PagePrerenderResult {
        page_type: "homepage".to_string(),
        output_path: PathBuf::from("/dist/index.html"),
    };
    let dbg = format!("{:?}", r);
    assert!(dbg.contains("PagePrerenderResult"));
    assert!(dbg.contains("homepage"));
    assert!(dbg.contains("index.html"));
}

#[test]
fn generated_feed_debug_clone() {
    let f = GeneratedFeed {
        filename: "feed.xml".to_string(),
        xml: "<rss/>".to_string(),
        item_count: 7,
    };
    let cloned = f.clone();
    assert_eq!(cloned.filename, "feed.xml");
    assert_eq!(cloned.item_count, 7);
    assert!(format!("{:?}", f).contains("GeneratedFeed"));
}

#[test]
fn escape_xml_handles_all_specials() {
    let escaped = escape_xml("a & b < c > d \"e\" 'f'");
    assert!(escaped.contains("&amp;"));
    assert!(escaped.contains("&lt;"));
    assert!(escaped.contains("&gt;"));
    assert!(escaped.contains("&quot;"));
    assert!(escaped.contains("&apos;"));
}

#[test]
fn build_sitemap_xml_with_no_urls_emits_empty_urlset() {
    let xml = build_sitemap_xml(&[]);
    assert!(xml.contains("<urlset"));
    assert!(xml.contains("</urlset>"));
}

#[test]
fn build_sitemap_xml_with_url() {
    let url = SitemapUrl {
        loc: "https://example.com/post".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.8,
        alternates: Vec::new(),
    };
    let xml = build_sitemap_xml(std::slice::from_ref(&url));
    assert!(xml.contains("https://example.com/post"));
    assert!(xml.contains("2025-01-01"));
    assert!(xml.contains("weekly"));
}

#[test]
fn build_sitemap_index_with_no_chunks() {
    let xml = build_sitemap_index(&[], "https://example.com");
    assert!(xml.contains("<sitemapindex"));
}

#[test]
fn build_sitemap_index_emits_chunks() {
    let chunk_a = vec![SitemapUrl {
        loc: "https://example.com/a".to_string(),
        lastmod: "2025-01-01".to_string(),
        changefreq: "weekly".to_string(),
        priority: 0.5,
        alternates: Vec::new(),
    }];
    let chunk_b = vec![SitemapUrl {
        loc: "https://example.com/b".to_string(),
        lastmod: "2025-01-02".to_string(),
        changefreq: "monthly".to_string(),
        priority: 0.4,
        alternates: Vec::new(),
    }];
    let xml = build_sitemap_index(&[chunk_a, chunk_b], "https://example.com");
    assert!(xml.contains("<sitemapindex"));
}

#[test]
fn build_rss_xml_with_empty_channel() {
    let ch = RssChannel {
        title: "Empty".to_string(),
        link: "https://x.test".to_string(),
        description: "desc".to_string(),
        items: Vec::new(),
    };
    let xml = build_rss_xml(&ch);
    assert!(xml.contains("<rss"));
    assert!(xml.contains("Empty"));
    assert!(xml.contains("</rss>"));
}
