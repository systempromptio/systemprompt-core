# Security Review

Review date: 2026-03-31
Codebase version: 0.1.18 (commit 38dcc3f50)
Reviewer: Automated deep audit

---

## CRITICAL-001: OAuth Redirect URI Bypass via Relative Path Matching

**Severity:** CRITICAL
**Category:** Authentication / OAuth
**File:** `crates/domain/oauth/src/services/validation/redirect_uri.rs:20-34`
**CVSS Estimate:** 9.1

### Description

The `matches_relative_uri()` function validates OAuth redirect URIs by extracting the path component from the requested URI and comparing it against registered relative URIs. The function strips the scheme and domain, then compares only the path.

### Vulnerable Code

```rust
fn matches_relative_uri(registered_uris: &[String], requested_uri: &str) -> bool {
    let requested_path = match requested_uri.find("://") {
        Some(scheme_end) => {
            let after_scheme = &requested_uri[scheme_end + 3..];
            after_scheme
                .find('/')
                .map_or("/", |slash_pos| &after_scheme[slash_pos..])
        },
        None => return false,
    };

    registered_uris.iter().any(|registered| {
        registered.starts_with('/') && !registered.starts_with("//") && registered == requested_path
    })
}
```

### Attack Scenario

1. A legitimate OAuth client registers the relative redirect URI `/callback`.
2. An attacker initiates an OAuth flow with `redirect_uri=https://attacker.com/callback`.
3. The function extracts `/callback` from the attacker URL and matches it against the registered `/callback`.
4. The authorization server issues an authorization code and redirects the user to `https://attacker.com/callback?code=VALID_CODE`.
5. The attacker exchanges the authorization code for access tokens.

### Impact

Complete OAuth flow compromise. An attacker can steal authorization codes and exchange them for access tokens, gaining full access to the victim's account and resources. This affects every OAuth client that registers a relative redirect URI.

### Remediation

Redirect URI validation must compare the full URI (scheme + host + path), never just the path component. If relative URIs are needed, they must be resolved against the client's registered origin before comparison.

```rust
fn matches_relative_uri(registered_uris: &[String], requested_uri: &str) -> bool {
    // Relative URIs should only match requests that are also relative
    // OR should be resolved against a pre-registered origin
    let requested_path = match requested_uri.find("://") {
        Some(_) => return false, // Full URIs cannot match relative registrations
        None => requested_uri,
    };

    registered_uris.iter().any(|registered| {
        registered.starts_with('/') && !registered.starts_with("//") && registered == requested_path
    })
}
```

Alternatively, remove relative URI support entirely and require all clients to register fully-qualified redirect URIs. This is the approach recommended by RFC 6749 Section 3.1.2.

---

## CRITICAL-002: User ID Spoofing in WebAuthn Complete Handler

**Severity:** CRITICAL
**Category:** Authentication / WebAuthn
**File:** `crates/entry/api/src/routes/oauth/endpoints/webauthn_complete.rs:70-89`
**CVSS Estimate:** 9.8

### Description

The WebAuthn completion handler accepts a `user_id` from an untrusted URL query parameter and issues OAuth tokens for that user without verifying that the WebAuthn authentication was performed by the same user. An attacker who completes WebAuthn authentication as themselves can request tokens for any arbitrary user.

### Vulnerable Code

```rust
pub async fn handle_webauthn_complete(
    headers: HeaderMap,
    Query(params): Query<WebAuthnCompleteQuery>,
    State(state): State<OAuthState>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(state.db_pool()) {
        // ...
    };
    // ...
    match user_provider.find_by_id(&params.user_id).await {
        Ok(Some(_)) => {
            // Issues token for ANY user_id from query param
```

### Attack Scenario

1. Attacker registers their own WebAuthn credential and authenticates normally.
2. Attacker intercepts or crafts the completion request, replacing `user_id` with the victim's user ID.
3. The handler looks up the victim user, confirms they exist, and issues an authorization code for the victim.
4. Attacker exchanges the code for tokens with full access to the victim's account.

### Impact

Complete account takeover. Any authenticated user can impersonate any other user in the system. This is the highest-severity authentication bypass possible — it requires no special privileges and affects every user.

### Remediation

The `user_id` must come from the verified WebAuthn authentication state, not from the query parameter. The WebAuthn authentication flow already associates a user with the challenge — that association must be the authoritative source.

```rust
// The user_id should come from the WebAuthn authentication result,
// not from the query parameter
let (authenticated_user_id, oauth_state) = webauthn_service
    .finish_authentication(challenge_id, &auth_response)
    .await?;

// Use authenticated_user_id, NOT params.user_id
match user_provider.find_by_id(&authenticated_user_id).await {
    // ...
}
```

---

## HIGH-001: Tarball Path Traversal via Symlinks

**Severity:** HIGH
**Category:** Input Validation / File Upload
**File:** `crates/entry/api/src/routes/sync/files.rs:117-159`
**CVSS Estimate:** 8.6

### Description

The `extract_tarball()` function unpacks uploaded tar.gz archives into the server's filesystem. It attempts to prevent path traversal by checking for `..` in path strings and validating the first path component against an allowlist. However, it does not handle symlink entries, encoded traversal sequences, or canonical path verification.

### Vulnerable Code

```rust
fn extract_tarball(data: &[u8], target: &Path) -> Result<usize, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    for entry in archive.entries()... {
        let mut entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path()...?;
        let entry_path_str = entry_path.to_string_lossy();
        if entry_path_str.contains("..") {
            return Err(format!("Invalid path in tarball: {}", entry_path_str));
        }

        let first_component = entry_path
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str());

        if !first_component.is_some_and(|c| ALLOWED_DIRS.contains(&c)) {
            return Err(format!("Path not in allowed directory: {}", entry_path_str));
        }

        let dest_path = target.join(&*entry_path);
        entry.unpack(&dest_path)...?;
    }
```

### Attack Vectors

1. **Symlink attack:** A tarball contains a symlink entry `allowed_dir/link -> /etc/` followed by a regular file `allowed_dir/link/crontab`. The symlink is created first, then the file write follows the symlink to `/etc/crontab`.
2. **Encoded traversal:** While unlikely in tar format, the string-based `..` check is fragile compared to canonical path resolution.
3. **Race condition:** Between path validation and file extraction, the filesystem state could change (TOCTOU).

### Impact

Arbitrary file write to any location on the server filesystem. This enables remote code execution via overwriting configuration files, cron jobs, SSH authorized keys, or application binaries.

### Remediation

```rust
fn extract_tarball(data: &[u8], target: &Path) -> Result<usize, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let canonical_target = target.canonicalize()
        .map_err(|e| format!("Failed to canonicalize target: {}", e))?;

    for entry in archive.entries()... {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;

        // Reject symlinks entirely
        if entry.header().entry_type().is_symlink()
            || entry.header().entry_type().is_hard_link() {
            return Err("Symlinks not allowed in tarball".to_string());
        }

        let entry_path = entry.path()...?;

        // Validate using canonical path resolution
        let dest_path = canonical_target.join(&*entry_path);
        let canonical_dest = dest_path.parent()
            .and_then(|p| std::fs::create_dir_all(p).ok().map(|_| p))
            .and_then(|p| p.canonicalize().ok())
            .ok_or("Failed to resolve destination")?;

        if !canonical_dest.starts_with(&canonical_target) {
            return Err(format!("Path escapes target directory: {}", entry_path.display()));
        }

        entry.unpack(&dest_path)...?;
    }
}
```

---

## HIGH-002: Environment Variable Leakage to Spawned Agent Processes

**Severity:** HIGH
**Category:** Secrets Management / Process Security
**File:** `crates/domain/agent/src/services/agent_orchestration/process.rs:86-109`
**CVSS Estimate:** 7.5

### Description

When spawning agent sub-processes, the code copies the entire parent process environment into each child process using `.envs(std::env::vars())`. This includes all secrets, API keys, database credentials, and any debugging or development variables present in the parent.

### Vulnerable Code

```rust
fn build_agent_command(
    binary_path: &PathBuf,
    agent_name: &str,
    port: u16,
    profile_path: &str,
    secrets: &Secrets,
    config: &Config,
    log_file: File,
) -> Command {
    let mut command = Command::new(binary_path);
    // ...
    command
        .envs(std::env::vars())  // Copies ALL parent environment variables
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("JWT_SECRET", &secrets.jwt_secret)
        .env("DATABASE_URL", &secrets.database_url)
        // ... explicitly sets more secrets on top
```

### Attack Scenario

1. Parent process has environment variables from `.env.secrets`, cloud provider metadata, or deployment configuration.
2. Each spawned agent inherits all of these, plus its own explicit secrets.
3. An attacker who compromises a single agent (via prompt injection, malicious MCP tool, etc.) can read `/proc/self/environ` or call `std::env::vars()` to harvest all secrets.
4. These secrets include database credentials, JWT signing keys, and API keys for Anthropic, OpenAI, Gemini, and GitHub.

### Impact

A single compromised agent process exposes all platform secrets. This enables lateral movement to the database, other API providers, and potentially the cloud infrastructure. With 50+ agents running, the attack surface is multiplied.

### Remediation

Replace `.envs(std::env::vars())` with an explicit allowlist of environment variables that agents actually need.

```rust
fn build_agent_command(/* ... */) -> Command {
    let mut command = Command::new(binary_path);
    // ...

    // Only pass explicitly required environment variables
    command
        .env_clear()  // Start with empty environment
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("JWT_SECRET", &secrets.jwt_secret)
        .env("DATABASE_URL", &secrets.database_url)
        // ... only the secrets this specific agent needs
```

---

## HIGH-003: Sync Token Timing Attack

**Severity:** HIGH
**Category:** Cryptography / Secrets
**File:** `crates/entry/api/src/routes/sync/auth.rs:29`
**CVSS Estimate:** 7.4

### Description

The sync authentication middleware compares the provided token against the expected token using Rust's standard `!=` operator. String comparison in Rust short-circuits on the first differing byte, making it vulnerable to timing side-channel attacks.

### Vulnerable Code

```rust
if provided_token != expected_token {
    return ApiError::unauthorized("Invalid sync token").into_response();
}
```

### Attack Scenario

An attacker sends thousands of requests with progressively guessed tokens, measuring response times with microsecond precision. The response is slightly faster when the first byte is wrong vs. when the first byte matches but the second is wrong. By iterating character-by-character, the attacker reconstructs the full SYNC_TOKEN.

### Impact

Full sync token recovery, enabling unauthorized access to the sync API. The sync API allows uploading tarballs (see HIGH-001), reading file manifests, and pushing/pulling content.

### Remediation

Use constant-time comparison. The `subtle` crate (already commonly used in Rust crypto) provides this:

```rust
use subtle::ConstantTimeEq;

let provided_bytes = provided_token.as_bytes();
let expected_bytes = expected_token.as_bytes();

if provided_bytes.len() != expected_bytes.len()
    || !bool::from(provided_bytes.ct_eq(expected_bytes))
{
    return ApiError::unauthorized("Invalid sync token").into_response();
}
```

Alternatively, compare HMAC digests of both tokens with a server-side key, which naturally provides constant-time comparison.

---

## HIGH-004: Weak PKCE Entropy Validation

**Severity:** HIGH
**Category:** OAuth / Cryptography
**File:** `crates/entry/api/src/routes/oauth/endpoints/authorize/validation.rs:175-197`
**CVSS Estimate:** 7.1

### Description

The PKCE code challenge entropy validation has multiple weaknesses. The `has_repeating_pattern()` function only checks if a pattern repeats from the start of the string. The `has_sequential_run()` function uses `to_digit(36)` which collapses case sensitivity. Together, these allow low-entropy challenges like `aaaaaaaaaa` or `abcdefghij` repeated to pass validation.

### Vulnerable Code

```rust
fn has_repeating_pattern(challenge: &str) -> bool {
    for pattern_length in 2..=4 {
        if challenge.len() >= pattern_length * 3 {
            let pattern = &challenge[..pattern_length];
            let repetitions = challenge.len() / pattern_length;
            if repetitions >= 3 {
                let repeated = pattern.repeat(repetitions);
                if challenge.starts_with(&repeated) {  // Only checks from START
                    return true;
                }
            }
        }
    }
    false
}
```

### Impact

Weak PKCE challenges reduce the security of the authorization code flow. If an attacker can predict or brute-force the code challenge, they can intercept authorization codes on public clients (mobile apps, SPAs) where PKCE is the primary defense against code interception.

### Remediation

Use Shannon entropy calculation instead of pattern detection heuristics:

```rust
fn calculate_shannon_entropy(s: &str) -> f64 {
    let len = s.len() as f64;
    let mut freq = std::collections::HashMap::new();
    for c in s.chars() {
        *freq.entry(c).or_insert(0u32) += 1;
    }
    freq.values()
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

fn validate_challenge_entropy(challenge: &str) -> Result<(), String> {
    let entropy = calculate_shannon_entropy(challenge);
    if entropy < 3.0 {  // Minimum 3 bits of entropy per character
        return Err("Code challenge has insufficient entropy".to_string());
    }
    Ok(())
}
```

---

## HIGH-005: Open Redirect in WebAuthn Complete Response

**Severity:** HIGH
**Category:** OAuth / Open Redirect
**File:** `crates/entry/api/src/routes/oauth/endpoints/webauthn_complete.rs:187-200`
**CVSS Estimate:** 6.8

### Description

After WebAuthn authentication completes, the handler redirects the browser to the `redirect_uri` without re-validating it against the OAuth client's registered URIs. The `redirect_uri` was validated during the `/authorize` step, but by the time `webauthn_complete` runs, no re-validation occurs.

### Vulnerable Code

```rust
if is_browser_request(headers) {
    let mut target = format!("{redirect_uri}?code={authorization_code}");
    if let Some(state) = state {
        target.push_str(&format!("&state={state}"));
    }
    Redirect::to(&target).into_response()
}
```

### Attack Scenario

1. Attacker starts a legitimate OAuth flow with a valid `redirect_uri`.
2. Attacker modifies the `redirect_uri` in the WebAuthn completion request (if it's passed through state or query parameters rather than server-side lookup).
3. The user completes WebAuthn authentication.
4. The server redirects the user to the attacker's URL with a valid authorization code.

### Impact

Authorization code theft via phishing redirect. The user sees a legitimate WebAuthn prompt, authenticates successfully, and is then silently redirected to an attacker-controlled site that harvests the code.

### Remediation

Store the `redirect_uri` server-side during the `/authorize` step and retrieve it from the server during completion. Never accept `redirect_uri` from the client in the completion flow.

---

## HIGH-006: No Rate Limiting on Authorization Endpoint

**Severity:** HIGH
**Category:** Denial of Service / OAuth
**File:** `crates/entry/api/src/routes/oauth/endpoints/authorize/handler.rs`
**CVSS Estimate:** 6.5

### Description

The `/authorize` endpoint has no visible rate limiting. Each request generates a new WebAuthn challenge, allocates server-side state, and stores it in memory. An attacker can spam this endpoint to exhaust memory or CPU.

### Impact

- Memory exhaustion via accumulated challenge states
- CPU exhaustion via bcrypt/PKCE operations
- Legitimate users unable to authenticate during attack
- Challenge state HashMap growth leading to degraded lookup performance

### Remediation

Apply rate limiting at both IP and client_id level:

```rust
// In route configuration
.route("/authorize", get(handle_authorize))
    .layer(rate_limit_layer(
        RateLimitConfig::new()
            .per_ip(10, Duration::from_secs(60))      // 10 req/min per IP
            .per_client(50, Duration::from_secs(60))   // 50 req/min per client
    ))
```

Additionally, enforce a maximum number of pending challenges and evict oldest when the limit is reached.

---

## HIGH-007: SSRF via OAuth Resource Parameter

**Severity:** HIGH
**Category:** SSRF / Input Validation
**File:** `crates/entry/api/src/routes/oauth/endpoints/authorize/validation.rs:158-173`
**CVSS Estimate:** 6.5

### Description

The `validate_resource_uri()` function accepts any HTTP or HTTPS URL as a resource parameter. It does not block internal IP addresses, localhost, link-local addresses, or cloud metadata endpoints.

### Vulnerable Code

```rust
fn validate_resource_uri(resource: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(resource)
        .map_err(|_| format!("Invalid resource URI: '{resource}' is not a valid absolute URI"))?;

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!(
            "Resource URI must use https or http scheme, got '{}'",
            url.scheme()
        ));
    }

    if url.fragment().is_some() {
        return Err("Resource URI must not contain a fragment".to_string());
    }

    Ok(())
}
```

### Attack Scenario

An attacker provides `resource=http://169.254.169.254/latest/meta-data/iam/security-credentials/` (AWS metadata endpoint) or `resource=http://127.0.0.1:5432` (local database). If the server later fetches or interacts with this URI, internal services are exposed.

### Impact

Access to cloud instance metadata (AWS/GCP/Azure credentials), internal services, and private network resources. Even if the URI is not fetched immediately, it may be used in later processing steps.

### Remediation

Add IP address validation that blocks internal ranges:

```rust
fn validate_resource_uri(resource: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(resource)
        .map_err(|_| format!("Invalid resource URI"))?;

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err("Resource URI must use https or http scheme".to_string());
    }

    if url.fragment().is_some() {
        return Err("Resource URI must not contain a fragment".to_string());
    }

    // Block internal/private addresses
    if let Some(host) = url.host_str() {
        if host == "localhost"
            || host == "127.0.0.1"
            || host == "::1"
            || host == "0.0.0.0"
            || host.starts_with("10.")
            || host.starts_with("172.16.")
            || host.starts_with("192.168.")
            || host.starts_with("169.254.")
            || host.ends_with(".internal")
            || host.ends_with(".local")
        {
            return Err("Resource URI must not point to internal addresses".to_string());
        }
    }

    Ok(())
}
```

---

## HIGH-008: CORS Wildcard on OAuth Endpoint

**Severity:** HIGH
**Category:** CORS / Cross-Origin
**File:** `crates/entry/api/src/routes/oauth/endpoints/webauthn_complete.rs:211-220`
**CVSS Estimate:** 6.1

### Description

The WebAuthn complete endpoint sets `Access-Control-Allow-Origin: *` which allows any website to make cross-origin requests to this security-sensitive endpoint.

### Vulnerable Code

```rust
headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
headers.insert("access-control-allow-methods", HeaderValue::from_static("GET, POST, OPTIONS"));
headers.insert("access-control-allow-headers", HeaderValue::from_static("content-type, authorization"));
```

### Impact

Any malicious website can make cross-origin requests to the WebAuthn completion endpoint. Combined with other vulnerabilities (CRITICAL-001, HIGH-005), this enables cross-site OAuth token theft without user interaction beyond visiting a malicious page.

### Remediation

Replace the wildcard with the specific allowed origin(s) from configuration:

```rust
if let Some(origin) = config.allowed_origins.first() {
    headers.insert("access-control-allow-origin", HeaderValue::from_str(origin)?);
}
headers.insert("vary", HeaderValue::from_static("Origin"));
```

---

## HIGH-009: WebAuthn Challenge Expiry Not Enforced

**Severity:** HIGH
**Category:** Authentication / WebAuthn
**File:** `crates/domain/oauth/src/services/webauthn/service/registration.rs:181-190`
**CVSS Estimate:** 5.9

### Description

WebAuthn registration and authentication states store a timestamp but never check it during retrieval. Expiration is handled only by an asynchronous cleanup task, creating a window where stale challenges remain valid.

### Vulnerable Code

```rust
async fn retrieve_and_remove_registration_state(
    &self,
    challenge_id: &str,
) -> Result<PasskeyRegistration> {
    let mut states = self.reg_states.lock().await;
    states
        .remove(challenge_id)
        .map(|(state, _timestamp)| state)  // Timestamp stored but NEVER checked
        .ok_or_else(|| anyhow::anyhow!("Registration state not found or expired"))
}
```

### Impact

An attacker who obtains a challenge ID (via log exposure, network interception, or brute force) has an indefinite window to complete the WebAuthn flow. Challenges should expire within 60-120 seconds to limit replay attacks.

### Remediation

Check the timestamp on retrieval:

```rust
async fn retrieve_and_remove_registration_state(
    &self,
    challenge_id: &str,
) -> Result<PasskeyRegistration> {
    let mut states = self.reg_states.lock().await;
    match states.remove(challenge_id) {
        Some((state, created_at)) => {
            let elapsed = created_at.elapsed().unwrap_or(Duration::MAX);
            if elapsed > Duration::from_secs(120) {
                return Err(anyhow::anyhow!("Registration challenge expired"));
            }
            Ok(state)
        }
        None => Err(anyhow::anyhow!("Registration state not found")),
    }
}
```

---

## MEDIUM-001: Unsafe Environment Variable Mutation

**Severity:** MEDIUM
**Category:** Thread Safety / Secrets
**File:** `crates/infra/config/src/services/manager.rs:97-100`
**CVSS Estimate:** 5.3

### Description

The configuration manager uses unsafe code to set environment variables from `.env.secrets` files. `std::env::set_var` is unsafe in Rust 2024 edition because it mutates global process state that may be read concurrently by other threads.

### Vulnerable Code

```rust
#[allow(unsafe_code)]
unsafe {
    std::env::set_var(key.trim(), value.trim().trim_matches('"'));
}
```

### Impact

- Race condition if multiple threads read environment variables concurrently during secret loading
- Secrets remain in the process environment for the entire lifetime, accessible via `/proc/[pid]/environ`
- Secrets are never scrubbed from memory after initialization

### Remediation

Replace environment variable mutation with a thread-safe secrets store:

```rust
use std::sync::OnceLock;
use std::collections::HashMap;

static SECRETS: OnceLock<HashMap<String, String>> = OnceLock::new();

pub fn load_secrets(path: &Path) -> Result<()> {
    let secrets = parse_env_file(path)?;
    SECRETS.set(secrets).map_err(|_| anyhow!("Secrets already loaded"))?;
    Ok(())
}

pub fn get_secret(key: &str) -> Option<&'static str> {
    SECRETS.get()?.get(key).map(|s| s.as_str())
}
```

---

## MEDIUM-002: OAuth State Not Validated Against WebAuthn User

**Severity:** MEDIUM
**Category:** Authentication / State Management
**File:** `crates/domain/oauth/src/services/webauthn/service/authentication.rs:65-67`
**CVSS Estimate:** 5.0

### Description

When WebAuthn authentication completes, the OAuth state parameter is retrieved from the WebAuthn challenge state. However, there is no validation that the user who started the OAuth flow is the same user who completed the WebAuthn authentication.

### Impact

OAuth state confusion attack. An attacker could start an OAuth flow as user A, then complete WebAuthn authentication as user B, potentially binding user B's session to user A's OAuth client.

### Remediation

Store the expected user identity in the OAuth state and validate it matches the WebAuthn-authenticated user after completion.

---

## MEDIUM-003: Error Messages Leak Internal Structure

**Severity:** MEDIUM
**Category:** Information Disclosure
**File:** `crates/shared/models/src/api/errors.rs:139-175`
**CVSS Estimate:** 4.3

### Description

API error responses include detailed information about internal structure: field names in validation errors, database operation names, and authentication method identifiers. While individually low-risk, this information aids attackers in mapping the application's internals.

### Impact

Reconnaissance aid. An attacker can enumerate valid field names, understand database schema, and identify authentication mechanisms by triggering different error conditions.

### Remediation

Return generic error messages for security-sensitive operations. Log detailed errors server-side for debugging. Only expose field-level detail for publicly-documented input validation (e.g., registration forms).

---

## MEDIUM-004: Authorization Code Error Message Enumeration

**Severity:** MEDIUM
**Category:** Information Disclosure
**File:** `crates/domain/oauth/src/repository/oauth/auth_code.rs:144-166`
**CVSS Estimate:** 4.0

### Description

Error messages from authorization code lookup distinguish between "code doesn't exist" and other failure modes, potentially allowing attackers to enumerate valid authorization codes via error message differences.

### Vulnerable Code

```rust
.map_err(|e| match e {
    sqlx::Error::RowNotFound => RepositoryError::NotFound(format!(
        "Context {} not found for user {}",
        context_id, user_id
    )),
    _ => RepositoryError::database(e),
})?;
```

### Impact

Authorization code enumeration via timing and error message differences. Low severity because codes are short-lived, but violates the principle of uniform error responses for security operations.

### Remediation

Return identical error responses for all authorization code failure modes:

```rust
.map_err(|_| RepositoryError::NotFound("Invalid authorization code".to_string()))?;
```

---

## MEDIUM-005: JWT Algorithm Hardcoded to HS256

**Severity:** MEDIUM
**Category:** Cryptography
**File:** `crates/infra/security/src/jwt/mod.rs:47`
**CVSS Estimate:** 3.7

### Description

JWT tokens are signed using HS256 (HMAC-SHA256) with a shared secret. While HS256 is not broken, asymmetric algorithms (RS256, ES256) provide better security properties for multi-service architectures because the verification key can be distributed without exposing the signing key.

### Impact

If the JWT secret is compromised (via environment variable leakage, see HIGH-002), an attacker can forge tokens for any user. With asymmetric signing, only the private key can create tokens, limiting the blast radius.

### Remediation

Consider migrating to ES256 (ECDSA) for new deployments. Support both HS256 and ES256 during a transition period. This is a long-term hardening measure, not an urgent fix.
