//! Sessions beyond a single run: where a session and its project live,
//! listings, forks, the agent and model a session remembers, the database
//! file and its failures, titles, skills and rules, requests for files outside
//! the project, child sessions and their requests, revert and diff.

mod changes;
mod children;
mod database;
mod forks;
mod helpers;
mod projects;
mod requests;
mod rules;
mod selection;
mod titles;
