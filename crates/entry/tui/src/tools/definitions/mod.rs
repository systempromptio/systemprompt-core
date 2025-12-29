mod database;
mod logs;
mod services;

pub use database::*;
pub use logs::*;
pub use services::*;

use std::sync::Arc;

use super::registry::TuiTool;
use super::ToolRegistry;

pub fn register_all_tools(registry: &mut ToolRegistry) {
    let tools: Vec<Arc<dyn TuiTool>> = vec![
        Arc::new(ServicesListTool),
        Arc::new(ServicesStatusTool),
        Arc::new(ServicesStartTool),
        Arc::new(ServicesStopTool),
        Arc::new(ServicesRestartTool),
        Arc::new(DbQueryTool),
        Arc::new(DbTablesTool),
        Arc::new(DbDescribeTool),
        Arc::new(LogsFilterTool),
        Arc::new(LogsSearchTool),
    ];

    for tool in tools {
        registry.register(tool);
    }
}
