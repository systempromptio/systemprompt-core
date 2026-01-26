use crate::error::ContentError;
use crate::models::ContentMetadata;

pub fn validate_content_metadata(metadata: &ContentMetadata) -> Result<(), ContentError> {
    if metadata.title.trim().is_empty() {
        return Err(ContentError::Validation(
            "title cannot be empty".to_string(),
        ));
    }

    if metadata.slug.trim().is_empty() {
        return Err(ContentError::Validation("slug cannot be empty".to_string()));
    }

    if metadata.author.trim().is_empty() {
        return Err(ContentError::Validation(
            "author cannot be empty".to_string(),
        ));
    }

    if metadata.published_at.trim().is_empty() {
        return Err(ContentError::Validation(
            "published_at cannot be empty".to_string(),
        ));
    }

    if !metadata
        .slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(ContentError::Validation(format!(
            "slug must be lowercase alphanumeric with hyphens only (got: {})",
            metadata.slug
        )));
    }

    if !is_valid_date_format(&metadata.published_at) {
        return Err(ContentError::Validation(format!(
            "published_at must be in YYYY-MM-DD format (got: {})",
            metadata.published_at
        )));
    }

    Ok(())
}

fn is_valid_date_format(date_str: &str) -> bool {
    if date_str.len() != 10 {
        return false;
    }

    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return false;
    }

    parts[0].len() == 4
        && parts[0].chars().all(char::is_numeric)
        && parts[1].len() == 2
        && parts[1].chars().all(char::is_numeric)
        && parts[2].len() == 2
        && parts[2].chars().all(char::is_numeric)
}
