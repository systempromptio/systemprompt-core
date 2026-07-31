//! Template validation for `web validate`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
            source: "templates".to_owned(),
            message: format!(
                "templates.yaml not found at {}",
                templates_yaml_path.display()
            ),
            suggestion: Some("Create a templates.yaml file".to_owned()),
        });
        return;
    }

    let Ok(content) = fs::read_to_string(&templates_yaml_path) else {
        errors.push(ValidationIssue {
            source: "templates".to_owned(),
            message: "Failed to read templates.yaml".to_owned(),
            suggestion: None,
        });
        return;
    };

    let Ok(templates_config) = serde_yaml::from_str::<TemplatesConfig>(&content) else {
        errors.push(ValidationIssue {
            source: "templates".to_owned(),
            message: "Failed to parse templates.yaml".to_owned(),
            suggestion: Some("Check YAML syntax".to_owned()),
        });
        return;
    };

    for name in templates_config.templates.keys() {
        let html_path = templates_dir.join(format!("{}.html", name));
        if !html_path.exists() {
            errors.push(ValidationIssue {
                source: "templates".to_owned(),
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

    let known = known_content_types(&content_config);
    warn_unknown_references(&templates_config, &known, warnings);
    warn_orphan_content_types(&templates_config, &known, warnings);
}

fn warn_unknown_references(
    templates_config: &TemplatesConfig,
    known: &HashSet<String>,
    warnings: &mut Vec<ValidationIssue>,
) {
    for (template_name, entry) in &templates_config.templates {
        for ct in &entry.content_types {
            if !known.contains(ct) {
                warnings.push(ValidationIssue {
                    source: "templates".to_owned(),
                    message: format!(
                        "Template '{}' references unknown content type '{}'",
                        template_name, ct
                    ),
                    suggestion: Some("Add the content type to content config".to_owned()),
                });
            }
        }
    }
}

fn warn_orphan_content_types(
    templates_config: &TemplatesConfig,
    known: &HashSet<String>,
    warnings: &mut Vec<ValidationIssue>,
) {
    let templated: HashSet<&str> = templates_config
        .templates
        .values()
        .flat_map(|e| e.content_types.iter())
        .map(String::as_str)
        .collect();

    let mut orphans: Vec<&str> = known
        .iter()
        .map(String::as_str)
        .filter(|name| !templated.contains(name))
        .collect();
    orphans.sort_unstable();

    for name in orphans {
        warnings.push(ValidationIssue {
            source: "templates".to_owned(),
            message: format!("Content type '{}' has no associated template", name),
            suggestion: Some("Link a template to this content type".to_owned()),
        });
    }
}

fn known_content_types(content_config: &ContentConfigRaw) -> HashSet<String> {
    let mut names: HashSet<String> = content_config
        .content_sources
        .values()
        .flat_map(|source| source.allowed_content_types.iter().cloned())
        .collect();

    for (source_name, source) in &content_config.content_sources {
        let renders_index = source
            .sitemap
            .as_ref()
            .and_then(|sitemap| sitemap.parent_route.as_ref())
            .is_some_and(|parent| parent.enabled);
        if renders_index {
            names.insert(format!("{source_name}-list"));
        }
    }

    names
}
