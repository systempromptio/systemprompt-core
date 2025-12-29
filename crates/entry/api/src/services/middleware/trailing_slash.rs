use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_models::modules::ApiPaths;

pub async fn remove_trailing_slash(request: Request, next: Next) -> Response {
    let uri = request.uri();
    let path = uri.path();

    if should_redirect(path) {
        let new_path = path.trim_end_matches('/');

        let new_uri = uri.query().map_or_else(
            || new_path.to_string(),
            |query| format!("{new_path}?{query}"),
        );

        return Redirect::permanent(&new_uri).into_response();
    }

    next.run(request).await
}

fn should_redirect(path: &str) -> bool {
    if path.len() <= 1 {
        return false;
    }

    if !path.ends_with('/') {
        return false;
    }

    if !path.starts_with(ApiPaths::API_BASE) {
        return false;
    }

    if path.starts_with(ApiPaths::WELLKNOWN_BASE) {
        return false;
    }

    if path.ends_with(".js/")
        || path.ends_with(".css/")
        || path.ends_with(".map/")
        || path.ends_with(".png/")
        || path.ends_with(".jpg/")
        || path.ends_with(".svg/")
        || path.ends_with(".ico/")
    {
        return false;
    }

    true
}
