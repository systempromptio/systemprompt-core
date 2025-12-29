/// Integration test module - Tests that interact with database and HTTP APIs
pub mod common;

// Sub-modules for each domain
pub mod a2a;
pub mod analytics;
pub mod agents;
pub mod auth;
pub mod content;
pub mod database;
pub mod files;
pub mod mcp;
pub mod models;
pub mod phase_1_security;
pub mod scheduler;
pub mod traits;
pub mod users;
