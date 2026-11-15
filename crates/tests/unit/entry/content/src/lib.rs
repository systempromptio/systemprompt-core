//! Tests for `systemprompt-content-entry`: the content route handlers and
//! route sets, the error mapping, and the
//! `core content` and `web sitemap` commands.

#[cfg(test)]
mod common;
#[cfg(test)]
mod core_content_analytics_db;
#[cfg(test)]
mod core_content_db;
#[cfg(test)]
mod core_content_dispatcher_db;
#[cfg(test)]
mod core_content_edit_apply;
#[cfg(test)]
mod core_content_edit_prompt_db;
#[cfg(test)]
mod core_content_files_db;
#[cfg(test)]
mod core_content_link_generate_db;
#[cfg(test)]
mod core_content_link_list_db;
#[cfg(test)]
mod core_content_link_ops_db;
#[cfg(test)]
mod core_content_public_mutation_lifecycle;
#[cfg(test)]
mod core_content_scope_db;
#[cfg(test)]
mod error_mapping;
#[cfg(test)]
mod http_blog_failures;
#[cfg(test)]
mod http_link_types;
#[cfg(test)]
mod http_redirect;
#[cfg(test)]
mod routes_content_blog;
#[cfg(test)]
mod routes_content_links;
#[cfg(test)]
mod routes_content_links_and_query;
#[cfg(test)]
mod routes_content_methods;
#[cfg(test)]
mod routes_content_more;
#[cfg(test)]
mod routes_content_negotiation;
#[cfg(test)]
mod web_sitemap_generate_fs;
#[cfg(test)]
mod web_sitemap_profile_fs;
#[cfg(test)]
mod web_sitemap_show;
