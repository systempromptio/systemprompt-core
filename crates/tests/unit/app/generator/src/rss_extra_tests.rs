use chrono::{TimeZone, Utc};
use systemprompt_generator::{GeneratedFeed, RssChannel, RssItem, build_rss_xml, escape_xml};

#[test]
fn generated_feed_fields_accessible() {
    let feed = GeneratedFeed {
        filename: "blog.xml".to_string(),
        xml: "<rss/>".to_string(),
        item_count: 3,
    };
    assert_eq!(feed.filename, "blog.xml");
    assert_eq!(feed.item_count, 3);
    assert!(feed.xml.contains("rss"));
}

#[test]
fn generated_feed_clone_is_independent() {
    let original = GeneratedFeed {
        filename: "orig.xml".to_string(),
        xml: "<rss/>".to_string(),
        item_count: 1,
    };
    let mut cloned = original.clone();
    cloned.filename = "cloned.xml".to_string();
    assert_eq!(original.filename, "orig.xml");
    assert_eq!(cloned.filename, "cloned.xml");
}

#[test]
fn generated_feed_debug_contains_filename() {
    let feed = GeneratedFeed {
        filename: "test-feed.xml".to_string(),
        xml: String::new(),
        item_count: 0,
    };
    let dbg = format!("{:?}", feed);
    assert!(dbg.contains("GeneratedFeed") || dbg.contains("test-feed.xml"));
}

#[test]
fn rss_channel_clone_is_independent() {
    let item = RssItem {
        title: "Post".to_string(),
        link: "https://example.com/post".to_string(),
        description: "Desc".to_string(),
        pub_date: Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap(),
        guid: "guid-1".to_string(),
        author: None,
    };
    let ch = RssChannel {
        title: "My Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Blog description".to_string(),
        items: vec![item],
    };
    let cloned = ch.clone();
    assert_eq!(cloned.title, ch.title);
    assert_eq!(cloned.items.len(), 1);
}

#[test]
fn rss_item_with_author_appears_in_xml() {
    let item = RssItem {
        title: "Author Test".to_string(),
        link: "https://example.com/post".to_string(),
        description: "A test post.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
        guid: "author-guid".to_string(),
        author: Some("Jane Smith".to_string()),
    };
    let ch = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "A blog.".to_string(),
        items: vec![item],
    };
    let xml = build_rss_xml(&ch);
    assert!(xml.contains("<author>Jane Smith</author>"));
}

#[test]
fn rss_item_without_author_omits_author_tag() {
    let item = RssItem {
        title: "No Author".to_string(),
        link: "https://example.com/post".to_string(),
        description: "No author post.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
        guid: "no-auth-guid".to_string(),
        author: None,
    };
    let ch = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "A blog.".to_string(),
        items: vec![item],
    };
    let xml = build_rss_xml(&ch);
    assert!(!xml.contains("<author>"));
}

#[test]
fn rss_item_special_chars_in_description_escaped() {
    let item = RssItem {
        title: "Special".to_string(),
        link: "https://example.com/special".to_string(),
        description: "See <https://example.com/page?a=1&b=2> for details.".to_string(),
        pub_date: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        guid: "spec-guid".to_string(),
        author: None,
    };
    let ch = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Blog.".to_string(),
        items: vec![item],
    };
    let xml = build_rss_xml(&ch);
    assert!(xml.contains("&lt;"));
    assert!(xml.contains("&amp;"));
}

#[test]
fn rss_many_items_all_appear_in_xml() {
    let items: Vec<RssItem> = (0..10)
        .map(|i| RssItem {
            title: format!("Post {i}"),
            link: format!("https://example.com/post/{i}"),
            description: format!("Description {i}"),
            pub_date: Utc
                .with_ymd_and_hms(2025, 1, i as u32 + 1, 0, 0, 0)
                .unwrap(),
            guid: format!("guid-{i}"),
            author: None,
        })
        .collect();
    let ch = RssChannel {
        title: "Blog".to_string(),
        link: "https://example.com".to_string(),
        description: "Blog.".to_string(),
        items,
    };
    let xml = build_rss_xml(&ch);
    for i in 0..10 {
        assert!(xml.contains(&format!("Post {i}")));
        assert!(xml.contains(&format!("/post/{i}")));
    }
    assert_eq!(xml.matches("<item>").count(), 10);
}

#[test]
fn rss_channel_link_in_atom_self_link() {
    let ch = RssChannel {
        title: "Blog".to_string(),
        link: "https://myblog.example.com".to_string(),
        description: "Blog.".to_string(),
        items: vec![],
    };
    let xml = build_rss_xml(&ch);
    assert!(xml.contains("href=\"https://myblog.example.com/feed.xml\""));
    assert!(xml.contains("type=\"application/rss+xml\""));
}

#[test]
fn escape_xml_no_specials_unchanged() {
    let plain = "Hello World, no special chars here";
    assert_eq!(escape_xml(plain), plain);
}

#[test]
fn escape_xml_empty_string() {
    assert_eq!(escape_xml(""), "");
}

#[test]
fn escape_xml_only_ampersands() {
    let s = "&&&";
    assert_eq!(escape_xml(s), "&amp;&amp;&amp;");
}

#[test]
fn escape_xml_mixed_specials() {
    let s = "a & b < c > d \"e\" 'f'";
    let escaped = escape_xml(s);
    assert!(escaped.contains("&amp;"));
    assert!(escaped.contains("&lt;"));
    assert!(escaped.contains("&gt;"));
    assert!(escaped.contains("&quot;"));
    assert!(escaped.contains("&apos;"));
    assert!(
        !escaped.contains('&')
            || escaped.contains("&amp;")
            || escaped.contains("&lt;")
            || escaped.contains("&gt;")
            || escaped.contains("&quot;")
            || escaped.contains("&apos;")
    );
}

#[test]
fn escape_xml_unicode_passthrough() {
    let s = "日本語 テスト 中文";
    assert_eq!(escape_xml(s), s);
}

#[test]
fn rss_channel_with_unicode_title_renders_correctly() {
    let ch = RssChannel {
        title: "Ünïcödé Blög".to_string(),
        link: "https://example.com".to_string(),
        description: "Unicode blog.".to_string(),
        items: vec![],
    };
    let xml = build_rss_xml(&ch);
    assert!(xml.contains("Ünïcödé Blög"));
}

#[test]
fn rss_pub_date_for_various_months() {
    let months = [
        (1, "Jan"),
        (2, "Feb"),
        (3, "Mar"),
        (4, "Apr"),
        (5, "May"),
        (6, "Jun"),
        (7, "Jul"),
        (8, "Aug"),
        (9, "Sep"),
        (10, "Oct"),
        (11, "Nov"),
        (12, "Dec"),
    ];
    for (month, abbr) in months {
        let item = RssItem {
            title: format!("Month {month}"),
            link: format!("https://example.com/{month}"),
            description: String::new(),
            pub_date: Utc.with_ymd_and_hms(2025, month, 1, 0, 0, 0).unwrap(),
            guid: format!("g-{month}"),
            author: None,
        };
        let ch = RssChannel {
            title: "Blog".to_string(),
            link: "https://example.com".to_string(),
            description: "Blog.".to_string(),
            items: vec![item],
        };
        let xml = build_rss_xml(&ch);
        assert!(
            xml.contains(abbr),
            "month {month} should produce '{abbr}' in pubDate"
        );
    }
}
