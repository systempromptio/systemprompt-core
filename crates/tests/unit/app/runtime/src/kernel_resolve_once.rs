//! One admitted composition resolves once: the first build of an
//! `AppContext` over it succeeds, and a second build over the same
//! `Admitted` is refused with `KernelError::AlreadyResolved` instead of
//! resolving a state without the slot values the first build consumed.

use systemprompt_runtime::{KernelError, RuntimeError};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted, fixture_app_context_with_extensions, fixture_config,
};

#[tokio::test]
async fn a_second_resolve_of_one_admitted_composition_is_refused() {
    let bootstrap = ensure_test_bootstrap();
    let admitted = fixture_admitted();

    let first = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        admitted.clone(),
    )
    .await;
    if let Err(error) = first {
        panic!("the first resolve builds the context: {error:#}");
    }

    let second =
        fixture_app_context_with_extensions(fixture_config(&bootstrap.database_url), admitted)
            .await;
    let Err(error) = second else {
        panic!("a second resolve over the same admitted composition must be refused");
    };
    assert!(
        matches!(
            error.downcast_ref::<RuntimeError>(),
            Some(RuntimeError::Kernel(KernelError::AlreadyResolved { .. }))
        ),
        "unexpected error: {error:#}"
    );
}
