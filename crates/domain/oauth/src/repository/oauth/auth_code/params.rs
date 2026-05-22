//! Parameter and result types for authorization-code persistence.

use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};

#[derive(Debug)]
pub struct AuthCodeParams<'a> {
    pub code: &'a AuthorizationCode,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub code_challenge: Option<&'a str>,
    pub code_challenge_method: Option<&'a str>,
    pub resource: Option<&'a str>,
}

#[derive(Debug)]
pub struct AuthCodeParamsBuilder<'a> {
    code: &'a AuthorizationCode,
    client_id: &'a ClientId,
    user_id: &'a UserId,
    redirect_uri: &'a str,
    scope: &'a str,
    code_challenge: Option<&'a str>,
    code_challenge_method: Option<&'a str>,
    resource: Option<&'a str>,
}

impl<'a> AuthCodeParamsBuilder<'a> {
    pub const fn new(
        code: &'a AuthorizationCode,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        redirect_uri: &'a str,
        scope: &'a str,
    ) -> Self {
        Self {
            code,
            client_id,
            user_id,
            redirect_uri,
            scope,
            code_challenge: None,
            code_challenge_method: None,
            resource: None,
        }
    }

    pub const fn with_pkce(mut self, challenge: &'a str, method: &'a str) -> Self {
        self.code_challenge = Some(challenge);
        self.code_challenge_method = Some(method);
        self
    }

    pub const fn with_resource(mut self, resource: &'a str) -> Self {
        self.resource = Some(resource);
        self
    }

    pub const fn build(self) -> AuthCodeParams<'a> {
        AuthCodeParams {
            code: self.code,
            client_id: self.client_id,
            user_id: self.user_id,
            redirect_uri: self.redirect_uri,
            scope: self.scope,
            code_challenge: self.code_challenge,
            code_challenge_method: self.code_challenge_method,
            resource: self.resource,
        }
    }
}

impl<'a> AuthCodeParams<'a> {
    pub const fn builder(
        code: &'a AuthorizationCode,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        redirect_uri: &'a str,
        scope: &'a str,
    ) -> AuthCodeParamsBuilder<'a> {
        AuthCodeParamsBuilder::new(code, client_id, user_id, redirect_uri, scope)
    }
}

#[derive(Debug)]
pub struct AuthCodeValidationResult {
    pub user_id: UserId,
    pub scope: String,
    pub resource: Option<String>,
}
