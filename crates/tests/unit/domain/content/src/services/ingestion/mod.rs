//! Unit tests for ingestion services
//!
//! Tests cover:
//! - validate_paper_frontmatter
//! - load_paper_chapters

use systemprompt_content::services::{load_paper_chapters, validate_paper_frontmatter};

// ============================================================================
// validate_paper_frontmatter Tests
// ============================================================================

#[test]
fn test_validate_paper_frontmatter_valid() {
    let markdown = r#"---
hero_image: /hero.png
sections:
  - id: intro
    title: Introduction
toc: true
---

# Content here
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_ok());
}

#[test]
fn test_validate_paper_frontmatter_missing_delimiters() {
    let markdown = r#"
hero_image: /hero.png
sections:
  - id: intro
    title: Introduction
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("frontmatter"));
}

#[test]
fn test_validate_paper_frontmatter_single_delimiter() {
    let markdown = r#"---
hero_image: /hero.png
sections:
  - id: intro
    title: Introduction
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
}

#[test]
fn test_validate_paper_frontmatter_empty_sections() {
    let markdown = r#"---
sections: []
---

# Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("at least one section"));
}

#[test]
fn test_validate_paper_frontmatter_duplicate_section_ids() {
    let markdown = r#"---
sections:
  - id: section1
    title: First Section
  - id: section1
    title: Duplicate Section
---

# Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Duplicate"));
}

#[test]
fn test_validate_paper_frontmatter_empty_section_id() {
    let markdown = r#"---
sections:
  - id: ""
    title: Empty ID Section
---

# Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("id cannot be empty"));
}

#[test]
fn test_validate_paper_frontmatter_empty_section_title() {
    let markdown = r#"---
sections:
  - id: section1
    title: ""
---

# Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must have a title"));
}

#[test]
fn test_validate_paper_frontmatter_invalid_yaml() {
    let markdown = r#"---
sections:
  - id: section1
    title: Valid
  invalid yaml here: [
---

# Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_err());
}

#[test]
fn test_validate_paper_frontmatter_multiple_sections() {
    let markdown = r#"---
hero_image: /images/hero.jpg
hero_alt: Hero description
sections:
  - id: chapter-1
    title: Chapter One
  - id: chapter-2
    title: Chapter Two
  - id: chapter-3
    title: Chapter Three
toc: true
---

# Paper Content
"#;

    let result = validate_paper_frontmatter(markdown);
    assert!(result.is_ok());
}

// ============================================================================
// load_paper_chapters Tests
// ============================================================================

#[test]
fn test_load_paper_chapters_no_chapters_path() {
    let markdown = r#"---
sections:
  - id: intro
    title: Introduction
---

# Main content here
"#;

    let result = load_paper_chapters(markdown);
    assert!(result.is_ok());
    // Should return original markdown when no chapters_path
    assert_eq!(result.unwrap(), markdown);
}

#[test]
fn test_load_paper_chapters_missing_delimiters() {
    let markdown = r#"
sections:
  - id: intro
    title: Introduction
"#;

    let result = load_paper_chapters(markdown);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("frontmatter"));
}

#[test]
fn test_load_paper_chapters_with_file_refs() {
    use std::fs;
    use std::io::Write;

    // Create temp directory with chapter files
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    // Create chapter file
    let chapter_file = temp_dir.path().join("chapter1.md");
    let mut file = fs::File::create(&chapter_file).unwrap();
    writeln!(file, "# Chapter 1\n\nThis is chapter 1 content.").unwrap();

    let markdown = format!(
        r#"---
sections:
  - id: ch1
    title: Chapter 1
    file: chapter1.md
chapters_path: {}
---

# Paper title
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("SECTION_START: ch1"));
    assert!(output.contains("chapter 1 content"));
    assert!(output.contains("SECTION_END: ch1"));
}

#[test]
fn test_load_paper_chapters_multiple_files() {
    use std::fs;
    use std::io::Write;

    // Create temp directory with chapter files
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    // Create chapter files
    for i in 1..=3 {
        let chapter_file = temp_dir.path().join(format!("chapter{}.md", i));
        let mut file = fs::File::create(&chapter_file).unwrap();
        writeln!(file, "# Chapter {}\n\nContent for chapter {}.", i, i).unwrap();
    }

    let markdown = format!(
        r#"---
sections:
  - id: ch1
    title: Chapter 1
    file: chapter1.md
  - id: ch2
    title: Chapter 2
    file: chapter2.md
  - id: ch3
    title: Chapter 3
    file: chapter3.md
chapters_path: {}
---
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should have all sections
    assert!(output.contains("SECTION_START: ch1"));
    assert!(output.contains("SECTION_END: ch1"));
    assert!(output.contains("SECTION_START: ch2"));
    assert!(output.contains("SECTION_END: ch2"));
    assert!(output.contains("SECTION_START: ch3"));
    assert!(output.contains("SECTION_END: ch3"));

    // Should preserve frontmatter
    assert!(output.contains("chapters_path:"));
}

#[test]
fn test_load_paper_chapters_mixed_file_refs() {
    use std::fs;
    use std::io::Write;

    // Create temp directory with one chapter file
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    let chapter_file = temp_dir.path().join("chapter1.md");
    let mut file = fs::File::create(&chapter_file).unwrap();
    writeln!(file, "# External Chapter\n\nExternal content.").unwrap();

    // Mix of file refs and inline sections
    let markdown = format!(
        r#"---
sections:
  - id: ch1
    title: Chapter 1
    file: chapter1.md
  - id: inline-section
    title: Inline Section
chapters_path: {}
---
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should include file-referenced chapter
    assert!(output.contains("SECTION_START: ch1"));
    assert!(output.contains("External content"));

    // Inline sections don't get wrapped
}

#[test]
fn test_load_paper_chapters_nonexistent_file() {
    // Create temp directory without chapter files
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    let markdown = format!(
        r#"---
sections:
  - id: ch1
    title: Chapter 1
    file: nonexistent.md
chapters_path: {}
---
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_err());
}

#[test]
fn test_load_paper_chapters_no_file_refs() {
    // Create temp directory (not used since no file refs)
    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    // Sections without file refs
    let markdown = format!(
        r#"---
sections:
  - id: intro
    title: Introduction
  - id: conclusion
    title: Conclusion
chapters_path: {}
---

# Inline content
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_ok());
    // Should return original markdown since no files to load
    assert_eq!(result.unwrap(), markdown);
}

#[test]
fn test_load_paper_chapters_preserves_frontmatter() {
    use std::fs;
    use std::io::Write;

    let temp_dir = tempfile::tempdir().unwrap();
    let chapters_path = temp_dir.path().to_str().unwrap();

    let chapter_file = temp_dir.path().join("ch.md");
    let mut file = fs::File::create(&chapter_file).unwrap();
    writeln!(file, "Chapter content").unwrap();

    let markdown = format!(
        r#"---
hero_image: /hero.jpg
hero_alt: Hero alt text
sections:
  - id: ch1
    title: Chapter
    file: ch.md
toc: true
chapters_path: {}
---
"#,
        chapters_path
    );

    let result = load_paper_chapters(&markdown);
    assert!(result.is_ok());
    let output = result.unwrap();

    // Should preserve all frontmatter fields
    assert!(output.contains("hero_image: /hero.jpg"));
    assert!(output.contains("hero_alt: Hero alt text"));
    assert!(output.contains("toc: true"));
}
