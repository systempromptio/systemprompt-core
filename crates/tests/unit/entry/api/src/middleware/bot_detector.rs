//! Unit tests for bot detector types
//!
//! Tests cover:
//! - BotType enum variants and equality
//! - BotMarker struct construction and field access
//! - Clone and Debug implementations

use systemprompt_api::services::middleware::bot_detector::{BotMarker, BotType};

// ============================================================================
// BotType Enum Tests
// ============================================================================

#[test]
fn test_bot_type_known_bot() {
    let bot_type = BotType::KnownBot;
    assert_eq!(bot_type, BotType::KnownBot);
}

#[test]
fn test_bot_type_scanner() {
    let bot_type = BotType::Scanner;
    assert_eq!(bot_type, BotType::Scanner);
}

#[test]
fn test_bot_type_suspicious() {
    let bot_type = BotType::Suspicious;
    assert_eq!(bot_type, BotType::Suspicious);
}

#[test]
fn test_bot_type_human() {
    let bot_type = BotType::Human;
    assert_eq!(bot_type, BotType::Human);
}

#[test]
fn test_bot_type_not_equal() {
    assert_ne!(BotType::KnownBot, BotType::Human);
    assert_ne!(BotType::Scanner, BotType::Suspicious);
    assert_ne!(BotType::Human, BotType::Scanner);
    assert_ne!(BotType::KnownBot, BotType::Scanner);
}

#[test]
fn test_bot_type_copy() {
    let original = BotType::KnownBot;
    let copied = original;
    assert_eq!(original, copied);
}

#[test]
fn test_bot_type_clone() {
    let original = BotType::Scanner;
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_bot_type_debug() {
    let bot_type = BotType::Human;
    let debug_str = format!("{:?}", bot_type);
    assert_eq!(debug_str, "Human");
}

#[test]
fn test_bot_type_all_variants_debug() {
    assert_eq!(format!("{:?}", BotType::KnownBot), "KnownBot");
    assert_eq!(format!("{:?}", BotType::Scanner), "Scanner");
    assert_eq!(format!("{:?}", BotType::Suspicious), "Suspicious");
    assert_eq!(format!("{:?}", BotType::Human), "Human");
}

// ============================================================================
// BotMarker Struct Tests
// ============================================================================

#[test]
fn test_bot_marker_known_bot() {
    let marker = BotMarker {
        is_bot: true,
        bot_type: BotType::KnownBot,
        user_agent: "Googlebot/2.1".to_string(),
        ip_address: None,
    };

    assert!(marker.is_bot);
    assert_eq!(marker.bot_type, BotType::KnownBot);
    assert_eq!(marker.user_agent, "Googlebot/2.1");
}

#[test]
fn test_bot_marker_human() {
    let marker = BotMarker {
        is_bot: false,
        bot_type: BotType::Human,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string(),
        ip_address: None,
    };

    assert!(!marker.is_bot);
    assert_eq!(marker.bot_type, BotType::Human);
}

#[test]
fn test_bot_marker_scanner() {
    let marker = BotMarker {
        is_bot: false,
        bot_type: BotType::Scanner,
        user_agent: "nmap scripting engine".to_string(),
        ip_address: None,
    };

    assert!(!marker.is_bot);
    assert_eq!(marker.bot_type, BotType::Scanner);
}

#[test]
fn test_bot_marker_suspicious() {
    let marker = BotMarker {
        is_bot: false,
        bot_type: BotType::Suspicious,
        user_agent: "suspicious-client".to_string(),
        ip_address: None,
    };

    assert!(!marker.is_bot);
    assert_eq!(marker.bot_type, BotType::Suspicious);
}

#[test]
fn test_bot_marker_empty_user_agent() {
    let marker = BotMarker {
        is_bot: false,
        bot_type: BotType::Human,
        user_agent: String::new(),
        ip_address: None,
    };

    assert!(marker.user_agent.is_empty());
}

#[test]
fn test_bot_marker_clone() {
    let original = BotMarker {
        is_bot: true,
        bot_type: BotType::KnownBot,
        user_agent: "bingbot".to_string(),
        ip_address: None,
    };

    let cloned = original.clone();
    assert_eq!(cloned.is_bot, original.is_bot);
    assert_eq!(cloned.bot_type, original.bot_type);
    assert_eq!(cloned.user_agent, original.user_agent);
}

#[test]
fn test_bot_marker_clone_independence() {
    let original = BotMarker {
        is_bot: true,
        bot_type: BotType::KnownBot,
        user_agent: "original".to_string(),
        ip_address: None,
    };

    let mut cloned = original.clone();
    cloned.is_bot = false;
    cloned.bot_type = BotType::Human;
    cloned.user_agent = "modified".to_string();

    assert!(original.is_bot);
    assert_eq!(original.bot_type, BotType::KnownBot);
    assert_eq!(original.user_agent, "original");
}

#[test]
fn test_bot_marker_debug() {
    let marker = BotMarker {
        is_bot: true,
        bot_type: BotType::Scanner,
        user_agent: "test-agent".to_string(),
        ip_address: None,
    };

    let debug_str = format!("{:?}", marker);
    assert!(debug_str.contains("BotMarker"));
    assert!(debug_str.contains("is_bot: true"));
    assert!(debug_str.contains("Scanner"));
    assert!(debug_str.contains("test-agent"));
}

// ============================================================================
// Real-World User Agent Examples
// ============================================================================

#[test]
fn test_bot_marker_common_bots() {
    let bot_agents = vec![
        ("Googlebot/2.1 (+http://www.google.com/bot.html)", true),
        ("bingbot/2.0 (+http://www.bing.com/bingbot.htm)", true),
        ("DuckDuckBot/1.0", true),
        ("facebookexternalhit/1.1", true),
        ("Twitterbot/1.0", true),
    ];

    for (user_agent, is_bot) in bot_agents {
        let marker = BotMarker {
            is_bot,
            bot_type: BotType::KnownBot,
            user_agent: user_agent.to_string(),
            ip_address: None,
        };
        assert!(marker.is_bot);
        assert_eq!(marker.bot_type, BotType::KnownBot);
    }
}

#[test]
fn test_bot_marker_common_browsers() {
    let browser_agents = vec![
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
        "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/121.0",
    ];

    for user_agent in browser_agents {
        let marker = BotMarker {
            is_bot: false,
            bot_type: BotType::Human,
            user_agent: user_agent.to_string(),
            ip_address: None,
        };
        assert!(!marker.is_bot);
        assert_eq!(marker.bot_type, BotType::Human);
    }
}
