//! Unit tests for RssItem and RssChannel types

use chrono::{TimeZone, Utc};
use systemprompt_generator::{RssChannel, RssItem};

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
