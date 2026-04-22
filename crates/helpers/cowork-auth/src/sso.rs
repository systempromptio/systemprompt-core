use std::env;

pub fn fetch_user_assertion() -> Result<String, String> {
    if let Ok(token) = env::var("SP_COWORK_USER_ASSERTION") {
        return Ok(token);
    }

    #[cfg(target_os = "macos")]
    {
        return Err("no user assertion available; install Okta Verify or set \
                    SP_COWORK_USER_ASSERTION"
            .to_string());
    }
    #[cfg(target_os = "windows")]
    {
        return Err("no user assertion available; AAD SSO not yet wired, set \
                    SP_COWORK_USER_ASSERTION"
            .to_string());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("SP_COWORK_USER_ASSERTION must be set on this platform".to_string())
    }
}
