use crate::error::ContentError;
use crate::models::{ContentMetadata, PaperMetadata};
use std::collections::HashSet;

pub fn validate_content_metadata(
    metadata: &ContentMetadata,
    allowed_types: &[&str],
) -> Result<(), ContentError> {
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

    if !allowed_types.contains(&metadata.kind.as_str()) {
        return Err(ContentError::Validation(format!(
            "invalid kind '{}'. must be one of: {}",
            metadata.kind,
            allowed_types.join(", ")
        )));
    }

    Ok(())
}

pub fn validate_paper_metadata(metadata: &PaperMetadata) -> Result<(), ContentError> {
    if metadata.sections.is_empty() {
        return Err(ContentError::Validation(
            "Paper must have at least one section".to_string(),
        ));
    }

    for section in &metadata.sections {
        if section.id.is_empty() {
            return Err(ContentError::Validation(
                "Section id cannot be empty".to_string(),
            ));
        }
        if section.title.is_empty() {
            return Err(ContentError::Validation(format!(
                "Section '{}' must have a title",
                section.id
            )));
        }
    }

    let has_file_refs = metadata.sections.iter().any(|s| s.file.is_some());

    if has_file_refs {
        let chapters_path = metadata.chapters_path.as_ref().ok_or_else(|| {
            ContentError::Validation(
                "chapters_path is required when sections reference files".to_string(),
            )
        })?;

        if chapters_path.is_empty() {
            return Err(ContentError::Validation(
                "chapters_path cannot be empty".to_string(),
            ));
        }

        let chapters_dir = std::path::Path::new(chapters_path);
        if !chapters_dir.exists() {
            return Err(ContentError::Validation(format!(
                "chapters_path '{}' does not exist",
                chapters_path
            )));
        }
        if !chapters_dir.is_dir() {
            return Err(ContentError::Validation(format!(
                "chapters_path '{}' is not a directory",
                chapters_path
            )));
        }

        for section in &metadata.sections {
            if let Some(file) = &section.file {
                let file_path = chapters_dir.join(file);
                if !file_path.exists() {
                    return Err(ContentError::Validation(format!(
                        "Section '{}' references file '{}' which does not exist at '{}'",
                        section.id,
                        file,
                        file_path.display()
                    )));
                }
                if !file_path.is_file() {
                    return Err(ContentError::Validation(format!(
                        "Section '{}' references '{}' which is not a file",
                        section.id, file
                    )));
                }
            }
        }
    }

    Ok(())
}

pub fn validate_paper_section_ids_unique(metadata: &PaperMetadata) -> Result<(), ContentError> {
    let mut seen_ids = HashSet::new();
    for section in &metadata.sections {
        if !seen_ids.insert(&section.id) {
            return Err(ContentError::Validation(format!(
                "Duplicate section id: '{}'",
                section.id
            )));
        }
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
