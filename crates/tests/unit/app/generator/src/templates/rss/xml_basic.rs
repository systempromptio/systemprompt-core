//! Unit tests for build_rss_xml basic functionality

use chrono::{TimeZone, Utc};
use systemprompt_generator::{build_rss_xml, RssChannel, RssItem};

#[test]
fn test_build_rss_xml_empty_channel() {
    let channel = RssChannel {
        title: "Empty Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "No posts yet.".to_string(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(result.contains("<rss version=\"2.0\""));
    assert!(result.contains("xmlns:atom=\"http://www.w3.org/2005/Atom\""));
    assert!(result.contains("<channel>"));
    assert!(result.contains("<title>Empty Blog</title>"));
    assert!(result.contains("<link>https://example.com</link>"));
    assert!(result.contains("<description>No posts yet.</description>"));
    assert!(result.contains("</channel>"));
    assert!(result.contains("</rss>"));
    assert!(!result.contains("<item>"));
}

#[test]
fn test_build_rss_xml_single_item() {
    let item = RssItem {
        title: "Test Article".to_string(),
        link: "https://example.com/test".to_string(),
        description: "A test article.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "https://example.com/test".to_string(),
        author: Some("John Doe".to_string()),
    };

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "My blog.".to_string(),
        items: vec![item],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("<item>"));
    assert!(result.contains("<title>Test Article</title>"));
    assert!(result.contains("<link>https://example.com/test</link>"));
    assert!(result.contains("<description>A test article.</description>"));
    assert!(result.contains("<pubDate>"));
    assert!(result.contains("<guid isPermaLink=\"true\">https://example.com/test</guid>"));
    assert!(result.contains("<author>John Doe</author>"));
    assert!(result.contains("</item>"));
}

#[test]
fn test_build_rss_xml_item_without_author() {
    let item = RssItem {
        title: "No Author".to_string(),
        link: "https://example.com/no-author".to_string(),
        description: "No author here.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "guid-123".to_string(),
        author: None,
    };

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description.".to_string(),
        items: vec![item],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("<item>"));
    assert!(!result.contains("<author>"));
}

#[test]
fn test_build_rss_xml_multiple_items() {
    let items = vec![
        RssItem {
            title: "First".to_string(),
            link: "https://example.com/first".to_string(),
            description: "First post.".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            guid: "guid-1".to_string(),
            author: None,
        },
        RssItem {
            title: "Second".to_string(),
            link: "https://example.com/second".to_string(),
            description: "Second post.".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 16, 10, 30, 0).unwrap(),
            guid: "guid-2".to_string(),
            author: None,
        },
        RssItem {
            title: "Third".to_string(),
            link: "https://example.com/third".to_string(),
            description: "Third post.".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 17, 10, 30, 0).unwrap(),
            guid: "guid-3".to_string(),
            author: None,
        },
    ];

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description.".to_string(),
        items,
    };

    let result = build_rss_xml(&channel);

    let item_count = result.matches("<item>").count();
    assert_eq!(item_count, 3);

    assert!(result.contains("<title>First</title>"));
    assert!(result.contains("<title>Second</title>"));
    assert!(result.contains("<title>Third</title>"));
}

#[test]
fn test_build_rss_xml_escapes_special_characters() {
    let channel = RssChannel {
        title: "Blog & News".to_string(),
        link: "https://example.com".to_string(),
        description: "Articles about <tech> & more".to_string(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("&amp;"));
    assert!(result.contains("&lt;tech&gt;"));
    assert!(!result.contains("<tech>") || result.contains("&lt;tech&gt;"));
}

#[test]
fn test_build_rss_xml_escapes_quotes() {
    let channel = RssChannel {
        title: "Blog \"Quotes\"".to_string(),
        link: "https://example.com".to_string(),
        description: "Test".to_string(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("&quot;"));
}

#[test]
fn test_build_rss_xml_escapes_apostrophe() {
    let channel = RssChannel {
        title: "John's Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Test".to_string(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("&apos;"));
}

#[test]
fn test_build_rss_xml_has_atom_link() {
    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description.".to_string(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("atom:link"));
    assert!(result.contains("href=\"https://example.com/feed.xml\""));
    assert!(result.contains("rel=\"self\""));
    assert!(result.contains("type=\"application/rss+xml\""));
}

#[test]
fn test_build_rss_xml_pub_date_format() {
    let item = RssItem {
        title: "Date Test".to_string(),
        link: "https://example.com/date".to_string(),
        description: "Testing date format.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "guid".to_string(),
        author: None,
    };

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description.".to_string(),
        items: vec![item],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("<pubDate>Mon, 15 Jan 2024 10:30:00 +0000</pubDate>"));
}
