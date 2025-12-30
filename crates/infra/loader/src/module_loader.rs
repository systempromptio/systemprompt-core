use systemprompt_models::Module;

use crate::modules;

#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    pub fn all() -> Vec<Module> {
        modules::all()
    }
}
