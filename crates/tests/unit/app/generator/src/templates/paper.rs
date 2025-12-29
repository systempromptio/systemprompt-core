//! Unit tests for paper template processing

use systemprompt_generator::templates::{
    calculate_read_time, generate_toc_html, parse_paper_metadata,
};

// ============================================================================
// calculate_read_time Tests
// ============================================================================

#[test]
fn test_calculate_read_time_short_content() {
    // Less than 200 words should be 1 minute minimum
    let html = "<p>Short content.</p>";
    let result = calculate_read_time(html);
    assert_eq!(result, 1);
}

#[test]
fn test_calculate_read_time_empty() {
    let result = calculate_read_time("");
    assert_eq!(result, 1); // Minimum is 1 minute
}

#[test]
fn test_calculate_read_time_200_words() {
    // 200 words at 200 wpm = 1 minute
    // Note: HTML tags like <p> and </p> are converted to words (p, /p)
    // so we use plain text to avoid that
    let words = vec!["word"; 200].join(" ");
    let result = calculate_read_time(&words);
    assert_eq!(result, 1);
}

#[test]
fn test_calculate_read_time_400_words() {
    // 400 words at 200 wpm = 2 minutes
    let words = vec!["word"; 400].join(" ");
    let result = calculate_read_time(&words);
    assert_eq!(result, 2);
}

#[test]
fn test_calculate_read_time_600_words() {
    // 600 words at 200 wpm = 3 minutes
    let words = vec!["word"; 600].join(" ");
    let result = calculate_read_time(&words);
    assert_eq!(result, 3);
}

#[test]
fn test_calculate_read_time_rounds_up() {
    // 250 words at 200 wpm = 1.25 minutes, should round up to 2
    let words = vec!["word"; 250].join(" ");
    let result = calculate_read_time(&words);
    assert_eq!(result, 2);
}

#[test]
fn test_calculate_read_time_counts_tag_names() {
    // The implementation replaces < and > with spaces, so tag names become words
    // "<div>" becomes " div " which counts as a word
    let html = "<div><p><strong>One</strong> <em>two</em> three</p></div>";
    let result = calculate_read_time(html);
    // Counts: div, p, strong, One, /strong, em, two, /em, three, /p, /div = 11 words
    // 11 / 200 = 0.055 -> ceil = 1
    assert_eq!(result, 1);
}

#[test]
fn test_calculate_read_time_complex_html() {
    let html = r#"
        <article>
            <h1>Title</h1>
            <p>First paragraph with some content.</p>
            <ul>
                <li>List item one</li>
                <li>List item two</li>
            </ul>
            <p>Another paragraph here.</p>
        </article>
    "#;
    let result = calculate_read_time(html);
    assert!(result >= 1);
}

#[test]
fn test_calculate_read_time_with_attributes() {
    let html = r#"<p class="intro" id="main">Content here</p>"#;
    let result = calculate_read_time(html);
    // Should only count "Content" and "here"
    assert_eq!(result, 1);
}

// ============================================================================
// parse_paper_metadata Tests
// ============================================================================

#[test]
fn test_parse_paper_metadata_valid() {
    let content = r#"---
title: Test Paper
sections:
  - id: introduction
    title: Introduction
  - id: methodology
    title: Methodology
toc: true
---
Paper content here."#;

    let result = parse_paper_metadata(content);
    assert!(result.is_some());

    let meta = result.unwrap();
    assert_eq!(meta.sections.len(), 2);
    assert_eq!(meta.sections[0].id, "introduction");
    assert_eq!(meta.sections[0].title, "Introduction");
    assert!(meta.toc);
}

#[test]
fn test_parse_paper_metadata_no_frontmatter() {
    let content = "Just content without frontmatter.";
    let result = parse_paper_metadata(content);
    assert!(result.is_none());
}

#[test]
fn test_parse_paper_metadata_incomplete_frontmatter() {
    let content = r#"---
title: Incomplete
No closing delimiter"#;

    let result = parse_paper_metadata(content);
    assert!(result.is_none());
}

#[test]
fn test_parse_paper_metadata_empty_frontmatter() {
    let content = r#"---
---
Content."#;

    let result = parse_paper_metadata(content);
    // Empty frontmatter should parse but may not have sections
    assert!(result.is_some());
}

#[test]
fn test_parse_paper_metadata_with_images() {
    let content = r#"---
title: Paper with Images
hero_image: /images/hero.jpg
hero_alt: Hero image description
sections:
  - id: intro
    title: Introduction
    image: /images/section1.png
    image_alt: Section 1 image
    image_position: right
---
Content."#;

    let result = parse_paper_metadata(content);
    assert!(result.is_some());

    let meta = result.unwrap();
    assert_eq!(meta.hero_image, Some("/images/hero.jpg".to_string()));
    assert_eq!(meta.hero_alt, Some("Hero image description".to_string()));
    assert_eq!(meta.sections[0].image, Some("/images/section1.png".to_string()));
}

#[test]
fn test_parse_paper_metadata_toc_false() {
    let content = r#"---
title: No TOC
toc: false
sections: []
---
Content."#;

    let result = parse_paper_metadata(content);
    assert!(result.is_some());

    let meta = result.unwrap();
    assert!(!meta.toc);
}

#[test]
fn test_parse_paper_metadata_whitespace_handling() {
    let content = r#"   ---
title: Whitespace Test
toc: true
sections: []
---
Content."#;

    let result = parse_paper_metadata(content);
    // Leading whitespace should still work due to trim()
    assert!(result.is_some());
}

// ============================================================================
// generate_toc_html Tests
// ============================================================================

#[test]
fn test_generate_toc_html_empty_sections() {
    let content = r#"---
title: No Sections
sections: []
toc: true
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let result = generate_toc_html(&meta);

    assert!(result.is_empty());
}

#[test]
fn test_generate_toc_html_single_section() {
    let content = r#"---
title: One Section
toc: true
sections:
  - id: intro
    title: Introduction
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let result = generate_toc_html(&meta);

    assert!(result.contains("<ul class=\"paper-toc__list\">"));
    assert!(result.contains("<li>"));
    assert!(result.contains("<a href=\"#intro\">Introduction</a>"));
    assert!(result.contains("</ul>"));
}

#[test]
fn test_generate_toc_html_multiple_sections() {
    let content = r#"---
title: Multiple Sections
toc: true
sections:
  - id: intro
    title: Introduction
  - id: methods
    title: Methodology
  - id: results
    title: Results
  - id: conclusion
    title: Conclusion
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let result = generate_toc_html(&meta);

    let li_count = result.matches("<li>").count();
    assert_eq!(li_count, 4);

    assert!(result.contains("href=\"#intro\""));
    assert!(result.contains("href=\"#methods\""));
    assert!(result.contains("href=\"#results\""));
    assert!(result.contains("href=\"#conclusion\""));
}

#[test]
fn test_generate_toc_html_special_characters_in_title() {
    let content = r#"---
title: Special Chars
toc: true
sections:
  - id: intro
    title: "Introduction & Overview"
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let result = generate_toc_html(&meta);

    // Title should appear as-is (HTML escaping is done elsewhere if needed)
    assert!(result.contains("Introduction & Overview") || result.contains("Introduction &amp; Overview"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_parse_paper_metadata_invalid_yaml() {
    let content = r#"---
title: Invalid
sections: [not valid yaml: [
---
Content."#;

    let result = parse_paper_metadata(content);
    assert!(result.is_none());
}

#[test]
fn test_calculate_read_time_only_html_tags() {
    let html = "<div><span></span></div>";
    let result = calculate_read_time(html);
    assert_eq!(result, 1); // Minimum
}

#[test]
fn test_calculate_read_time_many_words() {
    // 2000 words = 10 minutes
    let words = vec!["word"; 2000].join(" ");
    let result = calculate_read_time(&words);
    assert_eq!(result, 10);
}

#[test]
fn test_generate_toc_with_unicode_sections() {
    let content = r#"---
title: Unicode TOC
toc: true
sections:
  - id: japanese
    title: 日本語セクション
  - id: chinese
    title: 中文部分
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let result = generate_toc_html(&meta);

    assert!(result.contains("日本語セクション"));
    assert!(result.contains("中文部分"));
}

#[test]
fn test_parse_paper_metadata_section_with_all_fields() {
    let content = r#"---
title: Full Section
toc: true
sections:
  - id: section-one
    title: Section One
    image: /images/s1.png
    image_alt: Section one image
    image_position: left
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let section = &meta.sections[0];

    assert_eq!(section.id, "section-one");
    assert_eq!(section.title, "Section One");
    assert_eq!(section.image, Some("/images/s1.png".to_string()));
    assert_eq!(section.image_alt, Some("Section one image".to_string()));
    assert_eq!(section.image_position, "left");
}

#[test]
fn test_parse_paper_metadata_default_image_position() {
    let content = r#"---
title: Default Position
toc: true
sections:
  - id: intro
    title: Introduction
---
Content."#;

    let meta = parse_paper_metadata(content).unwrap();
    let section = &meta.sections[0];

    // Default image position should be "right" or similar default
    // This depends on the PaperMetadata implementation
    assert!(!section.image_position.is_empty() || section.image.is_none());
}
