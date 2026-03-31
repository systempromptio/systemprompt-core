use std::fs;

use systemprompt_models::content_config::ContentConfigRaw;

use super::super::types::ValidationIssue;

pub fn validate_sitemap(
    profile: &systemprompt_models::Profile,
    errors: &mut Vec<ValidationIssue>,
    warnings: &mut Vec<ValidationIssue>,
) {
    let content_config_path = profile.paths.content_config();

    let Ok(content) = fs::read_to_string(&content_config_path) else {
        return;
    };

    let Ok(content_config) = serde_yaml::from_str::<ContentConfigRaw>(&content) else {
        return;
    };

    let valid_changefreq = [
        "always", "hourly", "daily", "weekly", "monthly", "yearly", "never",
    ];

    for (name, source) in &content_config.content_sources {
        if let Some(sitemap) = &source.sitemap {
            if sitemap.priority < 0.0 || sitemap.priority > 1.0 {
                errors.push(ValidationIssue {
                    source: "sitemap".to_string(),
                    message: format!(
                        "Invalid priority {} for '{}' (must be 0.0-1.0)",
                        sitemap.priority, name
                    ),
                    suggestion: None,
                });
            }

            if !valid_changefreq.contains(&sitemap.changefreq.as_str()) {
                warnings.push(ValidationIssue {
                    source: "sitemap".to_string(),
                    message: format!(
                        "Invalid changefreq '{}' for '{}' (should be one of: {:?})",
                        sitemap.changefreq, name, valid_changefreq
                    ),
                    suggestion: None,
                });
            }

            if !sitemap.url_pattern.starts_with('/') {
                warnings.push(ValidationIssue {
                    source: "sitemap".to_string(),
                    message: format!("URL pattern for '{}' should start with '/'", name),
                    suggestion: Some(format!("Change to /{}", sitemap.url_pattern)),
                });
            }

            if let Some(parent) = &sitemap.parent_route {
                if parent.priority < 0.0 || parent.priority > 1.0 {
                    errors.push(ValidationIssue {
                        source: "sitemap".to_string(),
                        message: format!(
                            "Invalid parent route priority {} for '{}'",
                            parent.priority, name
                        ),
                        suggestion: None,
                    });
                }

                if !valid_changefreq.contains(&parent.changefreq.as_str()) {
                    warnings.push(ValidationIssue {
                        source: "sitemap".to_string(),
                        message: format!(
                            "Invalid parent route changefreq '{}' for '{}'",
                            parent.changefreq, name
                        ),
                        suggestion: None,
                    });
                }
            }
        }
    }
}
