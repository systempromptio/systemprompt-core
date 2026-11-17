//! Exported macros expanded in a crate whose only core dependency is the
//! `systemprompt` facade: a macro that names a core crate by an absolute path
//! instead of `$crate` fails to compile here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

#[cfg(test)]
mod expansion {
    use systemprompt::extension::router::{__IntoResponse, IntoApiError};
    use systemprompt::models::ApiError;

    struct ProbeError;

    impl IntoApiError for ProbeError {
        fn into_api_error(self) -> ApiError {
            ApiError::not_found("probe resource")
        }
    }

    systemprompt::extension::capability_error_response!(ProbeError);

    #[test]
    fn the_capability_error_response_expanded_through_the_facade_answers_with_the_api_error() {
        let response = __IntoResponse::into_response(ProbeError);
        assert_eq!(response.status().as_u16(), 404);
    }
}
