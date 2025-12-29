//! Unit tests for validation services
//!
//! Tests cover:
//! - validate_content_metadata
//! - validate_paper_metadata
//! - validate_paper_section_ids_unique
//! - is_valid_date_format (internal function behavior)

use systemprompt_core_content::models::{PaperMetadata, PaperSection};
use systemprompt_core_content::{
    validate_content_metadata, validate_paper_metadata, validate_paper_section_ids_unique,
    ContentMetadata,
};

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
// validate_paper_metadata Tests
// ============================================================================

#[test]
fn test_validate_paper_metadata_valid_minimal() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "intro".to_string(),
            title: "Introduction".to_string(),
            file: None,
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_metadata_empty_sections() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("at least one section"));
}

#[test]
fn test_validate_paper_metadata_empty_section_id() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "".to_string(),
            title: "Title".to_string(),
            file: None,
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("id cannot be empty"));
}

#[test]
fn test_validate_paper_metadata_empty_section_title() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "section-1".to_string(),
            title: "".to_string(),
            file: None,
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must have a title"));
}

#[test]
fn test_validate_paper_metadata_file_refs_without_chapters_path() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("chapters_path is required"));
}

#[test]
fn test_validate_paper_metadata_empty_chapters_path() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some("".to_string()),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("chapters_path cannot be empty"));
}

#[test]
fn test_validate_paper_metadata_nonexistent_chapters_path() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some("/nonexistent/path/12345".to_string()),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_validate_paper_metadata_multiple_valid_sections() {
    let metadata = PaperMetadata {
        hero_image: Some("/hero.png".to_string()),
        hero_alt: Some("Hero alt".to_string()),
        sections: vec![
            PaperSection {
                id: "section-1".to_string(),
                title: "Section 1".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "section-2".to_string(),
                title: "Section 2".to_string(),
                file: None,
                image: Some("/img.png".to_string()),
                image_alt: Some("Alt".to_string()),
                image_position: "left".to_string(),
            },
        ],
        toc: true,
        chapters_path: None,
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_ok());
}

// ============================================================================
// validate_paper_section_ids_unique Tests
// ============================================================================

#[test]
fn test_validate_paper_section_ids_unique_all_unique() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![
            PaperSection {
                id: "sec-1".to_string(),
                title: "Section 1".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "sec-2".to_string(),
                title: "Section 2".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "sec-3".to_string(),
                title: "Section 3".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
        ],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_section_ids_unique(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_section_ids_unique_duplicate() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![
            PaperSection {
                id: "duplicate-id".to_string(),
                title: "Section 1".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "unique-id".to_string(),
                title: "Section 2".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "duplicate-id".to_string(),
                title: "Section 3".to_string(),
                file: None,
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
        ],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_section_ids_unique(&metadata);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Duplicate section id"));
    assert!(err_msg.contains("duplicate-id"));
}

#[test]
fn test_validate_paper_section_ids_unique_single_section() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "only-section".to_string(),
            title: "Only Section".to_string(),
            file: None,
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_section_ids_unique(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_section_ids_unique_empty() {
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![],
        toc: false,
        chapters_path: None,
    };

    let result = validate_paper_section_ids_unique(&metadata);
    assert!(result.is_ok()); // Empty is trivially unique
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

// ============================================================================
// File System Validation Tests
// ============================================================================

#[test]
fn test_validate_paper_metadata_chapters_path_is_file() {
    // Create a temp file instead of a directory
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some(path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("is not a directory"));
}

#[test]
fn test_validate_paper_metadata_valid_with_file_refs() {
    use std::fs;
    use std::io::Write;

    // Create a temp directory with a chapter file
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap().to_string();

    // Create a chapter file
    let chapter_file = temp_dir.path().join("chapter1.md");
    let mut file = fs::File::create(&chapter_file).unwrap();
    writeln!(file, "# Chapter 1 content").unwrap();

    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some(chapters_path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_metadata_missing_referenced_file() {
    // Create a temp directory without the referenced file
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap().to_string();

    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("nonexistent.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some(chapters_path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("does not exist"));
    assert!(err_msg.contains("nonexistent.md"));
}

#[test]
fn test_validate_paper_metadata_file_ref_is_directory() {
    use std::fs;

    // Create a temp directory with a subdirectory instead of a file
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap().to_string();

    // Create a subdirectory instead of a file
    let subdir = temp_dir.path().join("chapter1.md");
    fs::create_dir(&subdir).unwrap();

    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![PaperSection {
            id: "ch1".to_string(),
            title: "Chapter 1".to_string(),
            file: Some("chapter1.md".to_string()),
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: false,
        chapters_path: Some(chapters_path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("is not a file"));
}

#[test]
fn test_validate_paper_metadata_multiple_files_valid() {
    use std::fs;
    use std::io::Write;

    // Create a temp directory with multiple chapter files
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap().to_string();

    // Create chapter files
    for i in 1..=3 {
        let chapter_file = temp_dir.path().join(format!("chapter{}.md", i));
        let mut file = fs::File::create(&chapter_file).unwrap();
        writeln!(file, "# Chapter {} content", i).unwrap();
    }

    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![
            PaperSection {
                id: "ch1".to_string(),
                title: "Chapter 1".to_string(),
                file: Some("chapter1.md".to_string()),
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "ch2".to_string(),
                title: "Chapter 2".to_string(),
                file: Some("chapter2.md".to_string()),
                image: None,
                image_alt: None,
                image_position: "left".to_string(),
            },
            PaperSection {
                id: "ch3".to_string(),
                title: "Chapter 3".to_string(),
                file: Some("chapter3.md".to_string()),
                image: None,
                image_alt: None,
                image_position: "center".to_string(),
            },
        ],
        toc: true,
        chapters_path: Some(chapters_path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_metadata_mixed_file_refs() {
    use std::fs;
    use std::io::Write;

    // Create a temp directory with some chapter files
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap().to_string();

    // Create only chapter1.md
    let chapter_file = temp_dir.path().join("chapter1.md");
    let mut file = fs::File::create(&chapter_file).unwrap();
    writeln!(file, "# Chapter 1 content").unwrap();

    // Section with file ref and section without
    let metadata = PaperMetadata {
        hero_image: None,
        hero_alt: None,
        sections: vec![
            PaperSection {
                id: "ch1".to_string(),
                title: "Chapter 1".to_string(),
                file: Some("chapter1.md".to_string()),
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
            PaperSection {
                id: "inline".to_string(),
                title: "Inline Section".to_string(),
                file: None, // No file reference
                image: None,
                image_alt: None,
                image_position: "right".to_string(),
            },
        ],
        toc: false,
        chapters_path: Some(chapters_path),
    };

    let result = validate_paper_metadata(&metadata);
    assert!(result.is_ok());
}
