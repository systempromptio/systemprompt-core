mod agent;
mod ai;
mod analytics;
mod api;
mod content;
mod database;
mod files;
mod log;
mod mcp;
mod oauth;
mod scheduler;
mod users;

use systemprompt_models::Module;

pub fn all() -> Vec<Module> {
    let mut modules = vec![
        database::define(),
        log::define(),
        users::define(),
        oauth::define(),
        mcp::define(),
        files::define(),
        content::define(),
        ai::define(),
        analytics::define(),
        agent::define(),
        scheduler::define(),
        api::define(),
    ];
    modules.sort_by_key(|m| m.weight.unwrap_or(100));
    modules
}
