//! Unit tests for validation services
//!
//! Tests cover:
//! - validate_content_metadata
//! - is_valid_date_format (internal function behavior)

use systemprompt_content::{validate_content_metadata, ContentMetadata};

// ============================================================================
// validate_content_metadata Tests
// ============================================================================

#[test]
fn test_validate_content_metadata_valid() {
    let metadata = ContentMetadata {
        title: "Valid Title".to_string(),
        description: "Valid description".to_string(),
        author: "John Doe".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "valid-slug".to_string(),
        keywords: "test, valid".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article", "paper", "guide"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_empty_title() {
    let metadata = ContentMetadata {
        title: "".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("title"));
}

#[test]
fn test_validate_content_metadata_whitespace_title() {
    let metadata = ContentMetadata {
        title: "   ".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_empty_slug() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("slug"));
}

#[test]
fn test_validate_content_metadata_empty_author() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("author"));
}

#[test]
fn test_validate_content_metadata_empty_published_at() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "".to_string(),
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("published_at"));
}

#[test]
fn test_validate_content_metadata_invalid_slug_uppercase() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "Invalid-Slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("slug"));
}

#[test]
fn test_validate_content_metadata_invalid_slug_spaces() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "invalid slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_valid_slug_with_numbers() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "article-2024-01".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_metadata_invalid_date_format() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "01-15-2024".to_string(), // Wrong format
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("YYYY-MM-DD"));
}

#[test]
fn test_validate_content_metadata_invalid_date_short() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-1-5".to_string(), // Should be 2024-01-05
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_metadata_invalid_kind() {
    let metadata = ContentMetadata {
        title: "Title".to_string(),
        description: "Desc".to_string(),
        author: "Author".to_string(),
        published_at: "2024-01-15".to_string(),
        slug: "slug".to_string(),
        keywords: "".to_string(),
        kind: "invalid-type".to_string(),
        image: None,
        category: None,
        tags: vec![],
        links: vec![],
    };

    let allowed_types = ["article", "paper"];
    let result = validate_content_metadata(&metadata, &allowed_types);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid kind"));
}

#[test]
fn test_validate_content_metadata_all_kinds() {
    let allowed_types = ["article", "paper", "guide", "tutorial"];

    for kind in &allowed_types {
        let metadata = ContentMetadata {
            title: "Title".to_string(),
            description: "Desc".to_string(),
            author: "Author".to_string(),
            published_at: "2024-01-15".to_string(),
            slug: "slug".to_string(),
            keywords: "".to_string(),
            kind: kind.to_string(),
            image: None,
            category: None,
            tags: vec![],
            links: vec![],
        };

        let result = validate_content_metadata(&metadata, &allowed_types);
        assert!(result.is_ok(), "Kind '{}' should be valid", kind);
    }
}

// ============================================================================
// Date Format Pattern Tests
// ============================================================================

#[test]
fn test_valid_date_formats() {
    let valid_dates = vec![
        "2024-01-01",
        "2024-12-31",
        "2000-06-15",
        "1999-01-01",
        "2099-12-31",
    ];

    for date in valid_dates {
        let metadata = ContentMetadata {
            title: "Title".to_string(),
            description: "Desc".to_string(),
            author: "Author".to_string(),
            published_at: date.to_string(),
            slug: "slug".to_string(),
            keywords: "".to_string(),
            kind: "article".to_string(),
            image: None,
            category: None,
            tags: vec![],
            links: vec![],
        };

        let allowed_types = ["article"];
        let result = validate_content_metadata(&metadata, &allowed_types);
        assert!(result.is_ok(), "Date '{}' should be valid", date);
    }
}

#[test]
fn test_invalid_date_formats() {
    // Test clearly invalid date formats that should fail validation
    let invalid_dates = vec![
        "2024/01/01",  // Wrong separator
        "24-01-01",    // Short year
        "2024-1-01",   // Single digit month
        "2024-01-1",   // Single digit day
        "01-01-2024",  // Wrong order
        "not-a-date",  // Completely invalid
        "2024",        // Just year
        "2024-01",     // Year and month only
    ];
    // Note: "2024-13-01" passes format check (YYYY-MM-DD pattern) but represents an invalid
    // date. The current validation only checks format, not logical validity.

    for date in invalid_dates {
        let metadata = ContentMetadata {
            title: "Title".to_string(),
            description: "Desc".to_string(),
            author: "Author".to_string(),
            published_at: date.to_string(),
            slug: "slug".to_string(),
            keywords: "".to_string(),
            kind: "article".to_string(),
            image: None,
            category: None,
            tags: vec![],
            links: vec![],
        };

        let allowed_types = ["article"];
        let result = validate_content_metadata(&metadata, &allowed_types);
        assert!(result.is_err(), "Date '{}' should be invalid", date);
    }
}
