use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct PlaybookPathInfo {
    pub playbook_id: String,
    pub category: String,
    pub domain: String,
    pub file_path: PathBuf,
}

pub fn playbook_id_to_path(playbooks_base: &Path, playbook_id: &str) -> Result<PathBuf> {
    let parts: Vec<&str> = playbook_id.split('_').collect();
    if parts.len() < 2 {
        return Err(anyhow!(
            "Invalid playbook ID '{}': must have at least category_name format",
            playbook_id
        ));
    }

    let mut path = playbooks_base.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            path.push(format!("{}.md", part));
        } else {
            path.push(part);
        }
    }

    Ok(path)
}

pub fn path_to_playbook_info(playbooks_base: &Path, file_path: &Path) -> Result<PlaybookPathInfo> {
    let relative = file_path
        .strip_prefix(playbooks_base)
        .map_err(|_| anyhow!("Path is not under playbooks base"))?;

    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.len() < 2 {
        return Err(anyhow!(
            "Path must have at least category/file.md structure"
        ));
    }

    let category = components[0].to_string();

    let filename = components[components.len() - 1];
    let domain_name = filename
        .strip_suffix(".md")
        .ok_or_else(|| anyhow!("File must have .md extension"))?;

    let domain_parts: Vec<&str> = components[1..components.len() - 1]
        .iter()
        .copied()
        .chain(std::iter::once(domain_name))
        .collect();
    let domain = domain_parts.join("/");

    let playbook_id = components
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == components.len() - 1 {
                c.strip_suffix(".md").unwrap_or(c)
            } else {
                *c
            }
        })
        .collect::<Vec<_>>()
        .join("_");

    Ok(PlaybookPathInfo {
        playbook_id,
        category,
        domain,
        file_path: file_path.to_path_buf(),
    })
}

pub fn domain_to_path_components(domain: &str) -> Vec<String> {
    domain.split('/').map(String::from).collect()
}

pub fn scan_all_playbooks(playbooks_base: &Path) -> Vec<PlaybookPathInfo> {
    if !playbooks_base.exists() {
        return Vec::new();
    }

    let mut playbooks = Vec::new();

    for entry in WalkDir::new(playbooks_base)
        .min_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        match path_to_playbook_info(playbooks_base, entry.path()) {
            Ok(info) => playbooks.push(info),
            Err(e) => {
                tracing::warn!(
                    path = %entry.path().display(),
                    error = %e,
                    "Skipping invalid playbook path"
                );
            },
        }
    }

    playbooks.sort_by(|a, b| a.playbook_id.cmp(&b.playbook_id));
    playbooks
}

pub fn validate_domain(domain: &str) -> Result<()> {
    if domain.starts_with('/') || domain.ends_with('/') {
        return Err(anyhow!("Domain cannot start or end with '/'"));
    }
    if domain.contains("..") {
        return Err(anyhow!("Domain cannot contain '..'"));
    }
    if domain.contains("//") {
        return Err(anyhow!("Domain cannot contain empty path segments"));
    }
    for part in domain.split('/') {
        if part.is_empty() {
            return Err(anyhow!("Domain cannot have empty path segments"));
        }
        if part.len() < 2 || part.len() > 30 {
            return Err(anyhow!(
                "Domain segment '{}' must be between 2 and 30 characters",
                part
            ));
        }
        if !part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(anyhow!(
                "Domain segment '{}' must be lowercase alphanumeric with hyphens only",
                part
            ));
        }
    }
    Ok(())
}

pub fn cleanup_empty_parent_dirs(playbooks_base: &Path, file_path: &Path) -> Result<()> {
    let mut current = file_path.parent();

    while let Some(dir) = current {
        if dir == playbooks_base {
            break;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            if entries.count() == 0 {
                std::fs::remove_dir(dir)?;
            } else {
                break;
            }
        } else {
            break;
        }

        current = dir.parent();
    }

    Ok(())
}
