//! Heuristic fix suggestions for common missing-frontmatter errors. Pulled
//! out of `error.rs` to keep that file focused on the error type itself.

pub(super) fn suggest_fix_for_field(field: &str) -> Option<String> {
    match field {
        "image" | "cover_image" => {
            Some("Add 'image: \"/files/images/placeholder.svg\"' to frontmatter".to_string())
        },
        "published_at" | "date" | "created_at" | "published_at/date/created_at" => {
            Some("Add 'date: YYYY-MM-DD' to frontmatter".to_string())
        },
        "author" => Some(
            "Add 'author: Your Name' to frontmatter or set metadata.default_author in config"
                .to_string(),
        ),
        "title" => Some("Add 'title: Your Title' to frontmatter".to_string()),
        "slug" => Some("Add 'slug: your-slug' to frontmatter".to_string()),
        "content_type" => {
            Some("Ensure content has a valid 'kind' field in frontmatter".to_string())
        },
        field if field.starts_with("organization.") => Some(format!(
            "Add '{}' under metadata.structured_data.organization in content.yaml",
            field.strip_prefix("organization.").unwrap_or(field)
        )),
        field if field.starts_with("article.") => Some(format!(
            "Add '{}' under metadata.structured_data.article in content.yaml",
            field.strip_prefix("article.").unwrap_or(field)
        )),
        field if field.starts_with("branding.") => Some(format!(
            "Add '{}' under branding in web.yaml",
            field.strip_prefix("branding.").unwrap_or(field)
        )),
        _ => None,
    }
}
