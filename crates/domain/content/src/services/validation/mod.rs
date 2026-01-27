use crate::error::ContentError;
use crate::models::ContentMetadata;

fn is_valid_slug_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn validate_slug(slug: &str) -> Result<(), ContentError> {
    if slug.is_empty() {
        return Ok(());
    }

    if slug.contains("//") {
        return Err(ContentError::Validation(format!(
            "slug cannot contain double slashes (got: {})",
            slug
        )));
    }

    let normalized = slug.trim_matches('/');
    if normalized.is_empty() {
        return Err(ContentError::Validation(
            "slug cannot consist of only slashes".to_string(),
        ));
    }

    for segment in normalized.split('/') {
        if !segment.is_empty() && !is_valid_slug_segment(segment) {
            return Err(ContentError::Validation(format!(
                "slug segment must be lowercase alphanumeric with hyphens only (got: {} in {})",
                segment, slug
            )));
        }
    }

    Ok(())
}

pub fn validate_content_metadata(metadata: &ContentMetadata) -> Result<(), ContentError> {
    if metadata.title.trim().is_empty() {
        return Err(ContentError::Validation(
            "title cannot be empty".to_string(),
        ));
    }

    validate_slug(&metadata.slug)?;

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
