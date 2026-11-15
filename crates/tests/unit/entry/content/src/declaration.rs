//! The `content-http` extension's three declarations, read from what the API
//! server mounts: the public content group and the signed-in link-generation
//! group at `/api/v1/content`, and the short-link redirect at the web root,
//! each in the content-and-misc role group under the `content` scope and
//! budget with no body limit, layers or frame options. The content
//! repositories they read come from the `content` extension's `provide`.

use std::sync::Arc;

use systemprompt_content::repository::ContentRepositories;
use systemprompt_content_entry::extension::ContentHttpExtension;
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, Extension, ExtensionRouter, RateBudget, RateLimit, RouteGroup,
    StateRequirement,
};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::offline_extension_context;
use systemprompt_traits::DynContentCatalogStats;

fn assert_shape(declared: &ExtensionRouter, base: &str, policy: AuthzPolicy, context: ContextKind) {
    assert_eq!(declared.base_path, base);
    assert_eq!(declared.group, RouteGroup::ContentAndMisc);
    assert_eq!(declared.protection.context, context);
    assert_eq!(declared.protection.policy.allowed(), policy.allowed());
    assert!(declared.protection.policy.handler_verifier().is_none());
    assert_eq!(
        declared.protection.rate_limit,
        Some(RateLimit::new("content", RateBudget::Content))
    );
    assert_eq!(declared.protection.body_limit, None);
    assert!(declared.protection.layers.is_empty());
    assert!(declared.protection.frame_options.is_none());
}

#[test]
fn the_extension_requires_the_repositories_content_provides() {
    let extension = ContentHttpExtension;
    assert_eq!(extension.metadata().id, "content-http");
    assert_eq!(extension.dependencies(), vec!["content"]);
    assert_eq!(
        extension.requires(),
        vec![StateRequirement::of::<Arc<ContentRepositories>>()]
    );
}

#[tokio::test]
async fn the_content_extension_provides_the_repositories_and_the_catalog() {
    let ctx = offline_extension_context();
    let state = ctx.extension_state();
    assert!(state.find::<Arc<ContentRepositories>>().is_some());
    assert!(state.find::<DynContentCatalogStats>().is_some());
}

#[tokio::test]
async fn the_three_groups_are_declared_with_their_core_table_shapes() {
    let ctx = offline_extension_context();
    let declarations = ContentHttpExtension
        .routers(&*ctx)
        .expect("the routes are declared over a state with the content repositories");
    assert_eq!(declarations.len(), 3);
    assert_shape(
        &declarations[0],
        ApiPaths::CONTENT_BASE,
        AuthzPolicy::public(),
        ContextKind::Public,
    );
    assert_shape(
        &declarations[1],
        ApiPaths::CONTENT_BASE,
        AuthzPolicy::user(),
        ContextKind::User,
    );
    assert_shape(
        &declarations[2],
        WEB_ROOT_BASE_PATH,
        AuthzPolicy::public(),
        ContextKind::Public,
    );
}
