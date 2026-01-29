use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_models::content_config::ContentConfigRaw;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::paths::WebPaths;
use super::types::{TemplatesConfig, ValidationIssue, ValidationOutput};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ValidationCategory {
    #[default]
    All,
    Config,
    Templates,
    Assets,
    Sitemap,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs {
    #[arg(long, value_enum, help = "Only check specific category")]
    pub only: Option<ValidationCategory>,
}

pub fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ValidationOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let web_paths = WebPaths::resolve_from_profile(profile)?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let category = args.only.unwrap_or(ValidationCategory::All);

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Config
    ) {
        validate_config(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Templates
    ) {
        validate_templates(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Assets
    ) {
        validate_assets(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Sitemap
    ) {
        validate_sitemap(profile, &mut errors, &mut warnings);
    }

    let valid = errors.is_empty();
    let items_checked = match category {
        ValidationCategory::All => 4,
        _ => 1,
    };

    let output = ValidationOutput {
        valid,
        items_checked,
        errors,
        warnings,
    };

    Ok(CommandResult::table(output).with_title("Web Configuration Validation"))
}

fn validate_config(
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

fn validate_templates(
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

fn validate_assets(
    profile: &systemprompt_models::Profile,
    web_paths: &WebPaths,
    errors: &mut Vec<ValidationIssue>,
    _warnings: &mut Vec<ValidationIssue>,
) {
    let web_config_path = profile.paths.web_config();
    let assets_dir = &web_paths.assets;

    if !assets_dir.exists() {
        return;
    }

    let Ok(config_content) = fs::read_to_string(&web_config_path) else {
        return;
    };

    let logo_refs = [
        "logo.svg",
        "logo.png",
        "logo.webp",
        "logo-dark.png",
        "logo-512.png",
    ];

    for logo in logo_refs {
        if config_content.contains(logo) {
            let logo_path = assets_dir.join("logos").join(logo);
            if !logo_path.exists() {
                errors.push(ValidationIssue {
                    source: "assets".to_string(),
                    message: format!("Referenced logo not found: {}", logo_path.display()),
                    suggestion: Some("Add the missing logo file".to_string()),
                });
            }
        }
    }

    if config_content.contains("favicon") {
        let favicon_path = assets_dir.join("favicon.ico");
        let favicon_svg = assets_dir.join("logos").join("logo.svg");
        if !favicon_path.exists() && !favicon_svg.exists() {
            errors.push(ValidationIssue {
                source: "assets".to_string(),
                message: "Referenced favicon not found".to_string(),
                suggestion: Some("Add a favicon.ico or logo.svg file".to_string()),
            });
        }
    }
}

fn validate_sitemap(
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
