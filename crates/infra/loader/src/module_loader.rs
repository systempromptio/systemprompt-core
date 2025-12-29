use anyhow::{bail, Result};
use std::path::Path;
use systemprompt_models::Module;

#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    pub fn scan_and_load(core_path: &str) -> Result<Vec<Module>> {
        let crates_dir = Path::new(core_path).join("crates");

        if !crates_dir.exists() {
            bail!("Crates directory not found: {}", crates_dir.display());
        }

        let module_categories = ["domain", "app", "infra"];

        let mut modules: Vec<Module> = module_categories
            .iter()
            .flat_map(|category| {
                let category_dir = crates_dir.join(category);
                Self::scan_category(&category_dir)
            })
            .collect();

        modules.sort_by_key(|m| m.weight.unwrap_or(100));

        Ok(modules)
    }

    fn scan_category(category_dir: &Path) -> Vec<Module> {
        if !category_dir.exists() {
            return Vec::new();
        }

        walkdir::WalkDir::new(category_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name() == "module.yaml")
            .filter_map(|entry| {
                Self::load_module_yaml(entry.path())
                    .map_err(|e| {
                        tracing::warn!(
                            path = %entry.path().display(),
                            error = %e,
                            "Error parsing module"
                        );
                        e
                    })
                    .ok()
            })
            .collect()
    }

    pub fn load_module_yaml(path: &Path) -> Result<Module> {
        let content = std::fs::read_to_string(path)?;
        let module_path = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Module::parse(&content, module_path)
    }

    pub fn exists(path: &Path) -> bool {
        path.join("module.yaml").exists()
    }
}
