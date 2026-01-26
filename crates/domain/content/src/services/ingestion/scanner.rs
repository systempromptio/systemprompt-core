use crate::error::ContentError;
use crate::models::ContentMetadata;
use crate::services::validation::validate_content_metadata;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ScanResult {
    pub files: Vec<PathBuf>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn scan_markdown_files(dir: &Path, recursive: bool) -> ScanResult {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let walker = if recursive {
        WalkDir::new(dir).min_depth(1)
    } else {
        WalkDir::new(dir).min_depth(1).max_depth(1)
    };

    let mut has_subdirectories = false;

    for entry in walker.into_iter().filter_map(Result::ok) {
        if entry.file_type().is_dir() && !recursive {
            has_subdirectories = true;
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let Some(ext) = entry.path().extension() else {
            continue;
        };

        if ext != "md" {
            continue;
        }

        match validate_markdown_file(entry.path()) {
            Ok(()) => files.push(entry.path().to_path_buf()),
            Err(e) => errors.push(format!("{}: {}", entry.path().display(), e)),
        }
    }

    if files.is_empty() && has_subdirectories {
        warnings.push(
            "No markdown files found in root directory, but subdirectories exist. Consider using \
             --recursive to scan nested directories."
                .to_string(),
        );
    }

    ScanResult {
        files,
        errors,
        warnings,
    }
}

fn validate_markdown_file(path: &Path) -> Result<(), ContentError> {
    let markdown_text = std::fs::read_to_string(path)?;
    let _ = parse_frontmatter(&markdown_text)?;
    Ok(())
}

pub fn parse_frontmatter(markdown: &str) -> Result<(ContentMetadata, String), ContentError> {
    let parts: Vec<&str> = markdown.splitn(3, "---").collect();

    if parts.len() < 3 {
        return Err(ContentError::Parse(
            "Invalid frontmatter format".to_string(),
        ));
    }

    let metadata: ContentMetadata = serde_yaml::from_str(parts[1])?;
    validate_content_metadata(&metadata)?;

    let content = parts[2].trim().to_string();

    Ok((metadata, content))
}
