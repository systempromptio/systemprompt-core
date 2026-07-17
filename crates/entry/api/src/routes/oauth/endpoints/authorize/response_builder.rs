//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AuthorizeQuery, AuthorizeRequest};
use std::collections::HashMap;
use systemprompt_models::Config;
use systemprompt_oauth::services::templating::TemplateEngine;

pub fn convert_form_to_query(form: &AuthorizeRequest) -> AuthorizeQuery {
    AuthorizeQuery {
        response_type: form.response_type.clone(),
        client_id: form.client_id.clone(),
        redirect_uri: form.redirect_uri.clone(),
        scope: form.scope.clone(),
        state: form.state.clone(),
        code_challenge: form.code_challenge.clone(),
        code_challenge_method: form.code_challenge_method.clone(),
        response_mode: None,
        display: None,
        prompt: None,
        max_age: None,
        ui_locales: None,
        resource: form.resource.clone(),
    }
}

pub fn is_user_consent_granted(form: &AuthorizeRequest) -> bool {
    form.user_consent.as_deref() == Some("allow")
}

pub fn generate_webauthn_form(params: &AuthorizeQuery, resolved_scope: &str) -> String {
    let template = TemplateEngine::load_webauthn_oauth_template();
    let mut context = HashMap::new();

    let redirect_uri = params.redirect_uri.as_deref().unwrap_or("");
    let state = params.state.as_deref().unwrap_or("");
    let code_challenge = params.code_challenge.as_deref().unwrap_or("");
    let code_challenge_method = params.code_challenge_method.as_deref().unwrap_or("");
    let resource = params.resource.as_deref().unwrap_or("");
    let api_external_url = Config::get().map_or("", |c| c.api_external_url.as_str());

    context.insert("client_id", params.client_id.as_str());
    context.insert("scope", resolved_scope);
    context.insert("response_type", params.response_type.as_str());
    context.insert("redirect_uri", redirect_uri);
    context.insert("state", state);
    context.insert("code_challenge", code_challenge);
    context.insert("code_challenge_method", code_challenge_method);
    context.insert("resource", resource);
    context.insert("api_external_url", api_external_url);

    let allow_registration = Config::get().map_or(true, |c| c.allow_registration);
    let register_class = if allow_registration { "" } else { "hidden" };
    context.insert("register_class", register_class);

    TemplateEngine::render(template, context)
}
