# OAuth Bug: MCP Server Scopes Lost During Dynamic Client Registration

## Summary

When an MCP server declares `scopes: [admin]` in its config, dynamically registered OAuth clients only receive the `user` scope. The `admin` scope is silently dropped, causing auth failures: `Scope 'admin' not allowed for client`.

## Error

```
http://localhost:62750/callback?error=invalid_request&error_description=Scope%20%27admin%27%20not%20allowed%20for%20client%20%27client_8dfade1090d14f41b261a15ee6194cac%27
```

## Database Evidence

```sql
-- MCP server config declares scopes: [admin]
-- But the registered client only has 'user':
SELECT c.client_id, c.client_name, s.scope
FROM oauth_clients c
JOIN oauth_client_scopes s ON c.client_id = s.client_id
WHERE c.client_name LIKE '%skills-editor%';

-- Result:
-- client_8dfade... | Claude Code (plugin:skills-admin:skills-editor) | user
```

## The Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│ 1. MCP Server Config (services/mcp/skills-editor.yaml)             │
│    oauth:                                                           │
│      required: true                                                 │
│      scopes: [admin]    ◄── admin scope declared here               │
│      audience: mcp                                                  │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 2. Protected Resource Metadata                                      │
│    GET /.well-known/oauth-protected-resource                        │
│    crates/entry/api/src/routes/proxy/mcp.rs:179-200                 │
│                                                                     │
│    get_mcp_server_scopes() reads OAuthRequirement.scopes            │
│    and converts Permission::Admin → "admin"                         │
│    ✓ Correctly exposes scopes: ["admin"]                            │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 3. Claude Code Sends Dynamic Registration Request                   │
│    POST /api/v1/core/oauth/register                                 │
│                                                                     │
│    DynamicRegistrationRequest {                                     │
│      client_name: "Claude Code (plugin:skills-admin:skills-editor)",│
│      scope: ???          ◄── Is Claude Code sending "admin" here?   │
│      redirect_uris: [...],                                          │
│      grant_types: ["authorization_code"],                           │
│    }                                                                │
│                                                                     │
│    QUESTION: Does Claude Code read the protected resource metadata  │
│    and populate scope in the registration request?                  │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 4. determine_scopes() — WHERE THE BUG LIVES                        │
│    crates/entry/api/src/routes/oauth/endpoints/register.rs:213-235  │
│                                                                     │
│    fn determine_scopes(request) {                                   │
│      if let Some(scope_string) = &request.scope {                   │
│        // If scope provided → validate and use it                   │
│        return Ok(validated_scopes);   // ← This path WORKS          │
│      }                                                              │
│                                                                     │
│      // If scope NOT provided → fall through to defaults            │
│      let default_roles = OAuthRepository::get_default_roles();      │
│      if default_roles.is_empty() {                                  │
│        Ok(vec!["user"])               // ← hardcoded fallback       │
│      } else {                                                       │
│        Ok(default_roles)              // ← returns ["user"]         │
│      }                                                              │
│    }                                                                │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 5. get_default_roles() — WHY "user" IS THE ONLY DEFAULT             │
│    crates/domain/oauth/src/repository/oauth/scopes.rs:60-66         │
│                                                                     │
│    const VALID_SCOPES: &[(&str, &str, bool)] = &[                   │
│      ("user",      "Standard user access",      true),  ← default   │
│      ("admin",     "Administrative access",     false), ← NOT default│
│      ("anonymous", "Anonymous user access",     false), ← NOT default│
│    ];                                                               │
│                                                                     │
│    get_default_roles() filters by is_default=true → ["user"]        │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 6. Client Created in DB with wrong scopes                           │
│    crates/domain/oauth/src/repository/client/mutations.rs:54-55     │
│                                                                     │
│    INSERT INTO oauth_client_scopes (client_id, scope)               │
│    SELECT $1, unnest($11::text[])                                   │
│                                                                     │
│    scopes = ["user"]     ◄── admin scope is gone                    │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 7. Authorization Fails                                              │
│    Client requests scope=admin at /authorize                        │
│    Server checks oauth_client_scopes → only "user" exists           │
│    → "Scope 'admin' not allowed for client"                         │
└─────────────────────────────────────────────────────────────────────┘
```

## Root Cause

There are two possible failure points, and both may contribute:

### Possibility A: Claude Code doesn't send scope in registration request

If Claude Code doesn't read the `/.well-known/oauth-protected-resource` metadata and include the required scopes in the `DynamicRegistrationRequest.scope` field, then `determine_scopes()` falls through to the default path and returns `["user"]`.

**Evidence:** All 3 registered clients for this MCP server have only `user` scope, suggesting the scope was never sent in any registration request.

### Possibility B: Server-side gap — no scope enforcement from MCP config

Even if Claude Code sends the correct scope, the registration endpoint has no way to verify that the requested scopes match what the MCP server requires. There is no server-side mechanism to:

1. Look up which MCP server the client is being registered for
2. Cross-reference the MCP server's declared `oauth.scopes`
3. Auto-assign or enforce those scopes on the client

The registration endpoint is generic — it has no awareness of MCP server configs.

## Key Files

| File | What it does |
|------|-------------|
| `crates/entry/api/src/routes/oauth/endpoints/register.rs:213-235` | `determine_scopes()` — defaults to `["user"]` when scope not provided |
| `crates/domain/oauth/src/repository/oauth/scopes.rs:4-8` | `VALID_SCOPES` — only `"user"` marked as default |
| `crates/domain/oauth/src/repository/oauth/scopes.rs:60-66` | `get_default_roles()` — returns `["user"]` |
| `crates/domain/oauth/src/repository/client/mutations.rs:54-55` | DB insert — stores whatever scopes are passed |
| `crates/entry/api/src/routes/proxy/mcp.rs:179-200` | `get_mcp_server_scopes()` — correctly exposes scopes via metadata |

## Suggested Fix

The registration endpoint should accept a server/resource identifier and auto-apply the required scopes from the MCP server config, rather than relying on the client to request the correct scopes. This would make scope assignment authoritative (server-side) rather than advisory (client-side).

Alternatively, if relying on the client to pass scopes, ensure Claude Code reads `/.well-known/oauth-protected-resource` and includes the declared scopes in the `DynamicRegistrationRequest.scope` field.
