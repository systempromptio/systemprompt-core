use std::collections::HashSet;
use std::fs;

use systemprompt_models::content_config::ContentConfigRaw;

use super::super::paths::WebPaths;
use super::super::types::{TemplatesConfig, ValidationIssue};

pub fn validate_templates(
    profile: &systemprompt_models::Profile,
    web_paths: &WebPaths,
    errors: &mut Vec<ValidationIssue>,
    warnings: &mut Vec<ValidationIssue>,
) {
    let templates_dir = &web_paths.templates;
    let templates_yaml_path = templates_dir.join("templates.yaml");

    if !templates_yaml_path.exists() {
        warnings.push(ValidationIssue {
            source: "templates".to_string(),
            message: format!(
                "templates.yaml not found at {}",
                templates_yaml_path.display()
            ),
            suggestion: Some("Create a templates.yaml file".to_string()),
        });
        return;
    }

    let Ok(content) = fs::read_to_string(&templates_yaml_path) else {
        errors.push(ValidationIssue {
            source: "templates".to_string(),
            message: "Failed to read templates.yaml".to_string(),
            suggestion: None,
        });
        return;
    };

    let Ok(templates_config) = serde_yaml::from_str::<TemplatesConfig>(&content) else {
        errors.push(ValidationIssue {
            source: "templates".to_string(),
            message: "Failed to parse templates.yaml".to_string(),
            suggestion: Some("Check YAML syntax".to_string()),
        });
        return;
    };

    for name in templates_config.templates.keys() {
        let html_path = templates_dir.join(format!("{}.html", name));
        if !html_path.exists() {
            errors.push(ValidationIssue {
                source: "templates".to_string(),
                message: format!("Missing HTML file for template '{}'", name),
                suggestion: Some(format!("Create {}", html_path.display())),
            });
        }
    }

    let content_config_path = profile.paths.content_config();
    let Ok(content) = fs::read_to_string(&content_config_path) else {
        return;
    };
    let Ok(content_config) = serde_yaml::from_str::<ContentConfigRaw>(&content) else {
        return;
    };

    let content_type_names: HashSet<&String> = content_config.content_sources.keys().collect();

    for (template_name, entry) in &templates_config.templates {
        for ct in &entry.content_types {
            if !content_type_names.contains(ct) {
                warnings.push(ValidationIssue {
                    source: "templates".to_string(),
                    message: format!(
                        "Template '{}' references unknown content type '{}'",
                        template_name, ct
                    ),
                    suggestion: Some("Add the content type to content config".to_string()),
                });
            }
        }
    }

    let template_content_types: HashSet<&String> = templates_config
        .templates
        .values()
        .flat_map(|e| e.content_types.iter())
        .collect();

    for name in content_type_names {
        if !template_content_types.contains(name) {
            warnings.push(ValidationIssue {
                source: "templates".to_string(),
                message: format!("Content type '{}' has no associated template", name),
                suggestion: Some("Link a template to this content type".to_string()),
            });
        }
    }
}
