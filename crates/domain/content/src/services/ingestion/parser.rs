use crate::error::ContentError;
use crate::models::PaperMetadata;
use crate::services::validation::{validate_paper_metadata, validate_paper_section_ids_unique};
use std::path::Path;

pub fn validate_paper_frontmatter(markdown: &str) -> Result<(), ContentError> {
    let parts: Vec<&str> = markdown.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(ContentError::Parse(
            "Invalid frontmatter format for paper".to_string(),
        ));
    }

    let paper_meta: PaperMetadata = serde_yaml::from_str(parts[1])?;

    validate_paper_metadata(&paper_meta)?;
    validate_paper_section_ids_unique(&paper_meta)?;

    Ok(())
}

pub fn load_paper_chapters(markdown: &str) -> Result<String, ContentError> {
    let parts: Vec<&str> = markdown.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(ContentError::Parse(
            "Invalid frontmatter format for paper".to_string(),
        ));
    }

    let frontmatter = parts[1];
    let paper_meta: PaperMetadata = serde_yaml::from_str(frontmatter)?;

    let Some(chapters_path) = &paper_meta.chapters_path else {
        return Ok(markdown.to_string());
    };

    let chapters_dir = Path::new(chapters_path);
    let mut chapter_content = String::new();

    for section in &paper_meta.sections {
        if let Some(file) = &section.file {
            let file_path = chapters_dir.join(file);
            let content = std::fs::read_to_string(&file_path)?;
            if !chapter_content.is_empty() {
                chapter_content.push_str("\n\n");
            }
            chapter_content.push_str(&format!(
                "<!-- SECTION_START: {} -->\n{}\n<!-- SECTION_END: {} -->",
                section.id,
                content.trim(),
                section.id
            ));
        }
    }

    if chapter_content.is_empty() {
        Ok(markdown.to_string())
    } else {
        Ok(format!("---\n{frontmatter}\n---\n\n{chapter_content}"))
    }
}
