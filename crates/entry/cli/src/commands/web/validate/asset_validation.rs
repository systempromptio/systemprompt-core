use std::fs;

use super::super::paths::WebPaths;
use super::super::types::ValidationIssue;

pub fn validate_assets(
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
