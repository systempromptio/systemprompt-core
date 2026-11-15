//! The `content-http` extension's three declarations, read from what the API
//! server mounts: the public content group and the signed-in link-generation
//! group at `/api/v1/content`, and the short-link redirect at the web root,
//! each in the content-and-misc role group under the `content` scope and
//! budget with no body limit, layers or frame options. The content
//! repositories they read come from the `content` extension's `provide`.
//! The real declarations pass the API's claim check, and declaring the
//! signed-in router with the public shape, dropping one or duplicating one
//! refuses boot. The real extension is the type its core rows name, so it
//! passes the owner check (this binary links the owning crate).

use std::sync::Arc;

use systemprompt_api::services::middleware::{
    CoreDeclaration, MountError, admit_core_owner, admit_extensions,
};
use systemprompt_content::repository::ContentRepositories;
use systemprompt_content_entry::extension::ContentHttpExtension;
use systemprompt_extension::registry::{ApiRouter, WEB_ROOT_BASE_PATH};
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

fn admitted(declarations: Vec<ExtensionRouter>) -> Result<(), MountError> {
    let extension: Arc<dyn Extension> = Arc::new(ContentHttpExtension);
    let mut declared: Vec<ApiRouter> = declarations
        .into_iter()
        .map(|router| (Arc::clone(&extension), router))
        .collect();
    admit_extensions(&mut declared)
}

#[tokio::test]
async fn the_real_declarations_each_claim_their_core_row_once() {
    let ctx = offline_extension_context();
    let declarations = ContentHttpExtension.routers(&*ctx).expect("declared");
    admitted(declarations).expect("each content row is claimed exactly once");
}

#[tokio::test]
async fn the_signed_in_router_declared_with_the_public_shape_refuses_boot() {
    let ctx = offline_extension_context();
    let mut declarations = ContentHttpExtension.routers(&*ctx).expect("declared");
    declarations[1].protection = declarations[0].protection.clone();
    let err = admitted(declarations).unwrap_err();
    assert!(
        matches!(
            err,
            MountError::CoreDeclarationClaimedTwice {
                extension: "content-http",
                row: CoreDeclaration::ContentPublic,
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_missing_or_duplicated_declaration_refuses_boot() {
    let ctx = offline_extension_context();
    let mut missing = ContentHttpExtension.routers(&*ctx).expect("declared");
    missing.remove(1);
    assert!(
        matches!(
            admitted(missing),
            Err(MountError::CoreDeclarationUnclaimed {
                row: CoreDeclaration::ContentAuthenticated,
                ..
            })
        ),
        "a missing signed-in router refuses boot"
    );

    let mut duplicated = ContentHttpExtension.routers(&*ctx).expect("declared");
    let again = ContentHttpExtension
        .routers(&*ctx)
        .expect("declared")
        .remove(2);
    duplicated.push(again);
    assert!(
        matches!(
            admitted(duplicated),
            Err(MountError::CoreDeclarationClaimedTwice {
                row: CoreDeclaration::ContentRedirect,
                ..
            })
        ),
        "a duplicated redirect router refuses boot"
    );
}

#[test]
fn the_content_extension_is_the_type_its_core_rows_name() {
    let ext: &dyn Extension = &ContentHttpExtension;
    for row in [
        CoreDeclaration::ContentPublic,
        CoreDeclaration::ContentAuthenticated,
        CoreDeclaration::ContentRedirect,
    ] {
        assert_eq!(row.owner_type(), Some(ext.type_name()), "{row:?}");
    }
    admit_core_owner(ext).expect("the real content extension owns its rows");
}
