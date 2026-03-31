use std::fs;
use std::path::Path;

use systemprompt_models::content_config::ContentConfigRaw;

use super::super::paths::WebPaths;
use super::super::types::ValidationIssue;

pub fn validate_config(
    profile: &systemprompt_models::Profile,
    web_paths: &WebPaths,
    errors: &mut Vec<ValidationIssue>,
    warnings: &mut Vec<ValidationIssue>,
) {
    let web_config_path = profile.paths.web_config();
    if !Path::new(&web_config_path).exists() {
        errors.push(ValidationIssue {
            source: "config".to_string(),
            message: format!("Web config not found at {}", web_config_path),
            suggestion: Some("Create a web config.yaml file".to_string()),
        });
    } else if let Err(e) = fs::read_to_string(&web_config_path) {
        errors.push(ValidationIssue {
            source: "config".to_string(),
            message: format!("Failed to read web config: {}", e),
            suggestion: None,
        });
    }

    let content_config_path = profile.paths.content_config();
    if !Path::new(&content_config_path).exists() {
        errors.push(ValidationIssue {
            source: "config".to_string(),
            message: format!("Content config not found at {}", content_config_path),
            suggestion: Some("Create a content config.yaml file".to_string()),
        });
        return;
    }

    let Ok(content) = fs::read_to_string(&content_config_path) else {
        errors.push(ValidationIssue {
            source: "config".to_string(),
            message: "Failed to read content config".to_string(),
            suggestion: None,
        });
        return;
    };

    let Ok(_content_config) = serde_yaml::from_str::<ContentConfigRaw>(&content) else {
        errors.push(ValidationIssue {
            source: "config".to_string(),
            message: "Failed to parse content config".to_string(),
            suggestion: Some("Check YAML syntax".to_string()),
        });
        return;
    };

    if !web_paths.templates.exists() {
        warnings.push(ValidationIssue {
            source: "config".to_string(),
            message: format!(
                "Templates directory not found: {}",
                web_paths.templates.display()
            ),
            suggestion: Some("Create the templates directory".to_string()),
        });
    }

    if !web_paths.assets.exists() {
        warnings.push(ValidationIssue {
            source: "config".to_string(),
            message: format!("Assets directory not found: {}", web_paths.assets.display()),
            suggestion: Some("Create the assets directory".to_string()),
        });
    }
}
