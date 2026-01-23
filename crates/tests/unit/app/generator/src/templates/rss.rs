//! Unit tests for RSS XML generation

use chrono::{TimeZone, Utc};
use systemprompt_generator::{build_rss_xml, RssChannel, RssItem};

// ============================================================================
// RssItem Tests
// ============================================================================

#[test]
fn test_rss_item_creation() {
    let item = RssItem {
        title: "Test Article".to_string(),
        link: "https://example.com/article".to_string(),
        description: "A test article description.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "https://example.com/article".to_string(),
        author: Some("John Doe".to_string()),
    };

    assert_eq!(item.title, "Test Article");
    assert_eq!(item.link, "https://example.com/article");
    assert_eq!(item.description, "A test article description.");
    assert_eq!(item.guid, "https://example.com/article");
    assert_eq!(item.author, Some("John Doe".to_string()));
}

#[test]
fn test_rss_item_without_author() {
    let item = RssItem {
        title: "No Author Article".to_string(),
        link: "https://example.com/no-author".to_string(),
        description: "Article without author.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "https://example.com/no-author".to_string(),
        author: None,
    };

    assert!(item.author.is_none());
}

#[test]
fn test_rss_item_clone() {
    let item = RssItem {
        title: "Cloneable".to_string(),
        link: "https://example.com/clone".to_string(),
        description: "Test clone.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "guid-123".to_string(),
        author: Some("Author".to_string()),
    };

    let cloned = item.clone();
    assert_eq!(item.title, cloned.title);
    assert_eq!(item.link, cloned.link);
    assert_eq!(item.guid, cloned.guid);
}

#[test]
fn test_rss_item_debug() {
    let item = RssItem {
        title: "Debug Test".to_string(),
        link: "https://example.com/debug".to_string(),
        description: "Test.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "guid".to_string(),
        author: None,
    };

    let debug = format!("{:?}", item);
    assert!(debug.contains("RssItem"));
    assert!(debug.contains("Debug Test"));
}

// ============================================================================
// RssChannel Tests
// ============================================================================

#[test]
fn test_rss_channel_creation() {
    let channel = RssChannel {
        title: "My Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "A blog about things.".to_string(),
        items: vec![],
    };

    assert_eq!(channel.title, "My Blog");
    assert_eq!(channel.link, "https://example.com");
    assert_eq!(channel.description, "A blog about things.");
    assert!(channel.items.is_empty());
}

#[test]
fn test_rss_channel_with_items() {
    let item = RssItem {
        title: "First Post".to_string(),
        link: "https://example.com/first".to_string(),
        description: "First post description.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
        guid: "https://example.com/first".to_string(),
        author: Some("Author".to_string()),
    };

    let channel = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Description".to_string(),
        items: vec![item],
    };

    assert_eq!(channel.items.len(), 1);
    assert_eq!(channel.items[0].title, "First Post");
}

#[test]
fn test_rss_channel_clone() {
    let channel = RssChannel {
        title: "Clone Test".to_string(),
        link: "https://example.com".to_string(),
        description: "Test.".to_string(),
        items: vec![],
    };

    let cloned = channel.clone();
    assert_eq!(channel.title, cloned.title);
}

#[test]
fn test_rss_channel_debug() {
    let channel = RssChannel {
        title: "Debug Channel".to_string(),
        link: "https://example.com".to_string(),
        description: "Test.".to_string(),
        items: vec![],
    };

    let debug = format!("{:?}", channel);
    assert!(debug.contains("RssChannel"));
    assert!(debug.contains("Debug Channel"));
}

// ============================================================================
// build_rss_xml Tests
// ============================================================================

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

// ============================================================================
// Edge Cases
// ============================================================================

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
