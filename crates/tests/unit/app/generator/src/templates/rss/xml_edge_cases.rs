//! Unit tests for build_rss_xml edge cases

use chrono::{TimeZone, Utc};
use systemprompt_generator::{RssChannel, RssItem, build_rss_xml};

#[test]
fn test_build_rss_xml_with_unicode() {
    let channel = RssChannel {
        title: "日本語ブログ".to_string(),
        link: "https://example.com".to_string(),
        description: "日本語の説明".to_string(),
        items: vec![RssItem {
            title: "日本語記事".to_string(),
            link: "https://example.com/japanese".to_string(),
            description: "これは日本語の記事です".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            guid: "jp-guid".to_string(),
            author: Some("山田太郎".to_string()),
        }],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("日本語ブログ"));
    assert!(result.contains("日本語記事"));
    assert!(result.contains("山田太郎"));
}

#[test]
fn test_build_rss_xml_with_long_content() {
    let long_description = "x".repeat(10000);
    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: long_description.clone(),
        items: vec![],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains(&long_description));
}

#[test]
fn test_build_rss_xml_item_with_empty_strings() {
    let item = RssItem {
        title: "".to_string(),
        link: "".to_string(),
        description: "".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "".to_string(),
        author: Some("".to_string()),
    };

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description.".to_string(),
        items: vec![item],
    };

    let result = build_rss_xml(&channel);

    assert!(result.contains("<title></title>"));
    assert!(result.contains("<author></author>"));
}

#[test]
fn test_build_rss_xml_well_formed() {
    let items = vec![
        RssItem {
            title: "Post 1".to_string(),
            link: "https://example.com/1".to_string(),
            description: "Desc 1".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            guid: "guid-1".to_string(),
            author: Some("Author".to_string()),
        },
        RssItem {
            title: "Post 2".to_string(),
            link: "https://example.com/2".to_string(),
            description: "Desc 2".to_string(),
            pub_date: Utc.with_ymd_and_hms(2024, 1, 16, 10, 30, 0).unwrap(),
            guid: "guid-2".to_string(),
            author: None,
        },
    ];

    let channel = RssChannel {
        title: "Test Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "A test blog.".to_string(),
        items,
    };

    let result = build_rss_xml(&channel);

    assert!(result.starts_with("<?xml"));
    assert!(result.ends_with("</rss>\n"));

    let item_open = result.matches("<item>").count();
    let item_close = result.matches("</item>").count();
    assert_eq!(item_open, item_close);

    let channel_open = result.matches("<channel>").count();
    let channel_close = result.matches("</channel>").count();
    assert_eq!(channel_open, 1);
    assert_eq!(channel_close, 1);
}
