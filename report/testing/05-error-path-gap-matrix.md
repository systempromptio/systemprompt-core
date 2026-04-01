# 05 - Error Path Gap Matrix

Generated: 2026-04-01

Catalogue of all production error enums, their variants, and test coverage status in `crates/tests/`.

## Summary Table

| # | Error Enum | File | Total | Covered | Uncovered | Coverage % |
|---|-----------|------|-------|---------|-----------|------------|
| 1 | `AuthError` | shared/models/src/auth/types.rs | 16 | 4 | 12 | 25% |
| 2 | `CoreError` | shared/models/src/errors.rs | 22 | 0 | 22 | 0% |
| 3 | `ServiceError` | shared/models/src/errors.rs | 8 | 0 | 8 | 0% |
| 4 | `InternalApiError` | shared/models/src/api/errors.rs | 12 | 0 | 12 | 0% |
| 5 | `ErrorCode` | shared/models/src/api/errors.rs | 9 | 0 | 9 | 0% |
| 6 | `McpError` | domain/mcp/src/error.rs | 11 | 11 | 0 | 100% |
| 7 | `AgentError` | domain/agent/src/error.rs | 6 | 6 | 0 | 100% |
| 8 | `TaskError` | domain/agent/src/error.rs | 12 | 11 | 1 | 92% |
| 9 | `ContextError` | domain/agent/src/error.rs | 6 | 5 | 1 | 83% |
| 10 | `ArtifactError` | domain/agent/src/error.rs | 8 | 7 | 1 | 88% |
| 11 | `ProtocolError` | domain/agent/src/error.rs | 8 | 7 | 1 | 88% |
| 12 | `RowParseError` | domain/agent/src/error.rs | 3 | 3 | 0 | 100% |
| 13 | `AgentServiceError` | domain/agent/src/services/shared/error.rs | 14 | 14 | 0 | 100% |
| 14 | `RepositoryError` (database) | infra/database/src/error.rs | 6 | 6 | 0 | 100% |
| 15 | `RepositoryError` (traits) | shared/traits/src/repository.rs | 6 | 0 | 6 | 0% |
| 16 | `ConfigError` | shared/extension/src/error.rs | 4 | 4 | 0 | 100% |
| 17 | `LoaderError` | shared/extension/src/error.rs | 9 | 9 | 0 | 100% |
| 18 | `CloudError` | infra/cloud/src/error.rs | 15 | 13 | 2 | 87% |
| 19 | `AiError` | domain/ai/src/error.rs | 22 | 21 | 1 | 95% |
| 20 | `RepositoryError` (ai) | domain/ai/src/error.rs | 4 | 4 | 0 | 100% |
| 21 | `UserError` | domain/users/src/error.rs | 7 | 6 | 1 | 86% |
| 22 | `ContentError` | domain/content/src/error.rs | 10 | 7 | 3 | 70% |
| 23 | `ContentValidationError` | domain/content/src/models/content_error.rs | 5 | 5 | 0 | 100% |
| 24 | `AnalyticsError` | domain/analytics/src/error.rs | 7 | 6 | 1 | 86% |
| 25 | `TemplateError` | domain/templates/src/error.rs | 6 | 6 | 0 | 100% |
| 26 | `OAuthParseError` | domain/oauth/src/models/oauth/mod.rs | 7 | 0 | 7 | 0% |
| 27 | `A2aParseError` | domain/agent/src/models/a2a/protocol/requests.rs | 2 | 0 | 2 | 0% |
| 28 | `IntegrationError` | domain/agent/src/models/external_integrations.rs | 11 | 9 | 2 | 82% |
| 29 | `TemplateLoaderError` | shared/template-provider/src/traits/error.rs | 9 | 9 | 0 | 100% |
| 30 | `ClientError` | shared/client/src/error.rs | 8 | 8 | 0 | 100% |
| 31 | `SchedulerError` (app) | app/scheduler/src/models/mod.rs | 7 | 7 | 0 | 100% |
| 32 | `SyncError` | app/sync/src/error.rs | 12 | 8 | 4 | 67% |
| 33 | `PublishError` | app/generator/src/error.rs | 7 | 0 | 7 | 0% |
| 34 | `BuildError` | app/generator/src/build/orchestrator.rs | 5 | 5 | 0 | 100% |
| 35 | `LoggingError` | infra/logging/src/models/log_error.rs | 17 | 15 | 2 | 88% |
| 36 | `TokenError` | entry/api/src/routes/oauth/endpoints/token/mod.rs | 9 | 0 | 9 | 0% |
| 37 | `TokenExtractionError` | infra/security/src/extraction/token.rs | 8 | 8 | 0 | 100% |
| 38 | `CookieExtractionError` | infra/security/src/extraction/cookie.rs | 3 | 3 | 0 | 100% |
| 39 | `QueryExecutorError` | infra/database/src/admin/query_executor.rs | 2 | 1 | 1 | 50% |
| 40 | `DatabaseSessionManagerError` | domain/mcp/src/middleware/session_manager.rs | 5 | 0 | 5 | 0% |
| 41 | `IdValidationError` | shared/identifiers/src/error.rs | 2 | 0 | 2 | 0% |
| 42 | `PathError` | shared/models/src/paths/error.rs | 6 | 0 | 6 | 0% |
| 43 | `ContextExtractionError` | shared/models/src/execution/context/context_error.rs | 11 | 0 | 11 | 0% |
| 44 | `ProfileBootstrapError` | shared/models/src/profile_bootstrap.rs | 5 | 0 | 5 | 0% |
| 45 | `SecretsBootstrapError` | shared/models/src/secrets_bootstrap.rs | 7 | 0 | 7 | 0% |
| 46 | `CredentialsBootstrapError` | infra/cloud/src/credentials_bootstrap.rs | 7 | 0 | 7 | 0% |
| 47 | `ContentConfigError` | shared/models/src/content_config.rs | 3 | 0 | 3 | 0% |
| 48 | `ValidationErrorKind` | shared/models/src/ai/template_validation.rs | 5 | 0 | 5 | 0% |
| 49 | `AiRequestRecordError` | domain/ai/src/models/ai_request_record.rs | 2 | 2 | 0 | 100% |
| 50 | `ConversionError` | shared/models/src/artifacts/cli/mod.rs | 4 | 0 | 4 | 0% |
| 51 | `RegistryError` | shared/traits/src/registry.rs | 4 | 0 | 4 | 0% |
| 52 | `SchedulerError` (traits) | shared/traits/src/scheduler.rs | 4 | 0 | 4 | 0% |
| 53 | `LlmProviderError` | shared/provider-contracts/src/llm.rs | 6 | 6 | 0 | 100% |
| 54 | `ToolProviderError` | shared/provider-contracts/src/tool.rs | 7 | 7 | 0 | 100% |
| 55 | `WebConfigError` | shared/provider-contracts/src/web_config/error.rs | 5 | 0 | 5 | 0% |
| 56 | `AuthProviderError` | shared/traits/src/auth.rs | 6 | 6 | 0 | 100% |
| 57 | `JwtProviderError` | shared/traits/src/jwt.rs | 5 | 0 | 5 | 0% |
| 58 | `DomainConfigError` | shared/traits/src/domain_config.rs | 5 | 0 | 5 | 0% |
| 59 | `FileUploadProviderError` | shared/traits/src/file_upload.rs | 4 | 0 | 4 | 0% |
| 60 | `AnalyticsProviderError` | shared/traits/src/analytics.rs | 3 | 0 | 3 | 0% |
| 61 | `McpServiceProviderError` | shared/traits/src/mcp_service.rs | 3 | 0 | 3 | 0% |
| 62 | `ContextProviderError` | shared/traits/src/context_provider.rs | 4 | 0 | 4 | 0% |
| 63 | `SessionAnalyticsProviderError` | shared/traits/src/session_analytics.rs | 2 | 0 | 2 | 0% |
| 64 | `AiProviderError` | shared/traits/src/ai_providers.rs | 5 | 0 | 5 | 0% |
| 65 | `ProcessProviderError` | shared/traits/src/process.rs | 4 | 0 | 4 | 0% |
| 66 | `ProfileResolutionError` | entry/cli/src/shared/profile.rs | 4 | 4 | 0 | 100% |
| 67 | `ProjectError` | entry/cli/src/shared/project.rs | 2 | 2 | 0 | 100% |

**Totals: 67 enums, 442 variants, 267 covered, 175 uncovered (60% overall)**

---

## Detailed Variant Coverage

### PRIORITY 1: High-Impact Uncovered Enums (Security / Auth / API)

#### 1. AuthError (shared/models/src/auth/types.rs) -- 25% covered

| Variant | Status |
|---------|--------|
| `InvalidTokenFormat` | UNCOVERED |
| `TokenExpired` | UNCOVERED |
| `InvalidSignature` | UNCOVERED |
| `UserNotFound` | UNCOVERED |
| `InsufficientPermissions` | UNCOVERED |
| `AuthenticationFailed` | UNCOVERED |
| `InvalidRequest` | COVERED |
| `MissingState` | COVERED |
| `InvalidRedirectUri` | COVERED |
| `MissingCodeChallenge` | UNCOVERED |
| `WeakPkceMethod` | UNCOVERED |
| `ClientNotFound` | UNCOVERED |
| `InvalidScope` | COVERED |
| `UnauthenticatedRevocation` | UNCOVERED |
| `InvalidRpId` | UNCOVERED |
| `RegistrationFailed` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 2. CoreError (shared/models/src/errors.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `MissingConfigField` | UNCOVERED |
| `InvalidVersion` | UNCOVERED |
| `InvalidModuleConfig` | UNCOVERED |
| `ModuleNotFound` | UNCOVERED |
| `InvalidField` | UNCOVERED |
| `VersionComparisonFailed` | UNCOVERED |
| `AuthenticationFailed` | UNCOVERED |
| `InvalidToken` | UNCOVERED |
| `TokenExpired` | UNCOVERED |
| `InvalidSignature` | UNCOVERED |
| `MissingClaim` | UNCOVERED |
| `InvalidAuthHeader` | UNCOVERED |
| `InvalidTokenFormat` | UNCOVERED |
| `Unauthorized` | UNCOVERED |
| `Forbidden` | UNCOVERED |
| `UserNotFound` | UNCOVERED |
| `SessionNotFound` | UNCOVERED |
| `InvalidSession` | UNCOVERED |
| `SessionExpired` | UNCOVERED |
| `CookieNotFound` | UNCOVERED |
| `InvalidCookieFormat` | UNCOVERED |
| `DatabaseError` | UNCOVERED |
| `TableNotFound` | UNCOVERED |
| `SchemaError` | UNCOVERED |
| `FileNotFound` | UNCOVERED |
| `IoError` | UNCOVERED |
| `InternalError` | UNCOVERED |

#### 3. ServiceError (shared/models/src/errors.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Repository` | UNCOVERED |
| `Validation` | UNCOVERED |
| `BusinessLogic` | UNCOVERED |
| `External` | UNCOVERED |
| `NotFound` | UNCOVERED |
| `Conflict` | UNCOVERED |
| `Unauthorized` | UNCOVERED |
| `Forbidden` | UNCOVERED |

#### 4. InternalApiError (shared/models/src/api/errors.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotFound` | UNCOVERED |
| `BadRequest` | UNCOVERED |
| `Unauthorized` | UNCOVERED |
| `Forbidden` | UNCOVERED |
| `ValidationError` | UNCOVERED |
| `ConflictError` | UNCOVERED |
| `RateLimited` | UNCOVERED |
| `ServiceUnavailable` | UNCOVERED |
| `DatabaseError` | UNCOVERED |
| `JsonError` | UNCOVERED |
| `AuthenticationError` | UNCOVERED |
| `InternalError` | UNCOVERED |

#### 5. TokenError (entry/api/src/routes/oauth/endpoints/token/mod.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `InvalidRequest` | UNCOVERED |
| `UnsupportedGrantType` | UNCOVERED |
| `InvalidClient` | UNCOVERED |
| `InvalidGrant` | UNCOVERED |
| `InvalidRefreshToken` | UNCOVERED |
| `InvalidCredentials` | UNCOVERED |
| `InvalidClientSecret` | UNCOVERED |
| `ExpiredCode` | UNCOVERED |
| `ServerError` | UNCOVERED |

#### 6. ContextExtractionError (shared/models/src/execution/context/context_error.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `MissingHeader` | UNCOVERED |
| `MissingAuthHeader` | UNCOVERED |
| `InvalidToken` | UNCOVERED |
| `MissingSessionId` | UNCOVERED |
| `MissingUserId` | UNCOVERED |
| `MissingContextId` | UNCOVERED |
| `InvalidHeaderValue` | UNCOVERED |
| `InvalidUserId` | UNCOVERED |
| `DatabaseError` | UNCOVERED |
| `UserNotFound` | UNCOVERED |
| `ForbiddenHeader` | UNCOVERED |

### PRIORITY 2: Infrastructure / Bootstrap (0% coverage)

#### 7. ProfileBootstrapError (shared/models/src/profile_bootstrap.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotInitialized` | UNCOVERED |
| `AlreadyInitialized` | UNCOVERED |
| `PathNotSet` | UNCOVERED |
| `ValidationFailed` | UNCOVERED |
| `LoadFailed` | UNCOVERED |

#### 8. SecretsBootstrapError (shared/models/src/secrets_bootstrap.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotInitialized` | UNCOVERED |
| `AlreadyInitialized` | UNCOVERED |
| `ProfileNotInitialized` | UNCOVERED |
| `FileNotFound` | UNCOVERED |
| `InvalidSecretsFile` | UNCOVERED |
| `NoSecretsConfigured` | UNCOVERED |
| `JwtSecretRequired` | UNCOVERED |
| `DatabaseUrlRequired` | UNCOVERED |

#### 9. CredentialsBootstrapError (infra/cloud/src/credentials_bootstrap.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotInitialized` | UNCOVERED |
| `AlreadyInitialized` | UNCOVERED |
| `NotAvailable` | UNCOVERED |
| `FileNotFound` | UNCOVERED |
| `InvalidCredentials` | UNCOVERED |
| `TokenExpired` | UNCOVERED |
| `ApiValidationFailed` | UNCOVERED |

#### 10. PathError (shared/models/src/paths/error.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotInitialized` | UNCOVERED |
| `AlreadyInitialized` | UNCOVERED |
| `NotConfigured` | UNCOVERED |
| `NotFound` | UNCOVERED |
| `CanonicalizeFailed` | UNCOVERED |
| `BinaryNotFound` | UNCOVERED |

#### 11. IdValidationError (shared/identifiers/src/error.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Empty` | UNCOVERED |
| `Invalid` | UNCOVERED |

#### 12. DatabaseSessionManagerError (domain/mcp/src/middleware/session_manager.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Local` | UNCOVERED |
| `Database` | UNCOVERED |
| `SessionNotFound` | UNCOVERED |
| `SessionExpired` | UNCOVERED |
| `SessionNeedsReconnect` | UNCOVERED |

### PRIORITY 3: Domain Parse/Validation Errors (0% coverage)

#### 13. OAuthParseError (domain/oauth/src/models/oauth/mod.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `GrantType` | UNCOVERED |
| `PkceMethod` | UNCOVERED |
| `ResponseType` | UNCOVERED |
| `ResponseMode` | UNCOVERED |
| `DisplayMode` | UNCOVERED |
| `Prompt` | UNCOVERED |
| `TokenAuthMethod` | UNCOVERED |

#### 14. A2aParseError (domain/agent/src/models/a2a/protocol/requests.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `UnsupportedMethod` | UNCOVERED |
| `InvalidParams` | UNCOVERED |

#### 15. ValidationErrorKind (shared/models/src/ai/template_validation.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `InvalidTemplateSyntax` | UNCOVERED |
| `IndexOutOfBounds` | UNCOVERED |
| `SelfReference` | UNCOVERED |
| `ForwardReference` | UNCOVERED |
| `FieldNotFound` | UNCOVERED |
| `NoOutputSchema` | UNCOVERED |

#### 16. ConversionError (shared/models/src/artifacts/cli/mod.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `MissingColumns` | UNCOVERED |
| `NoArrayFound` | UNCOVERED |
| `Json` | UNCOVERED |
| `UnsupportedType` | UNCOVERED |

#### 17. ContentConfigError (shared/models/src/content_config.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Io` | UNCOVERED |
| `Parse` | UNCOVERED |
| `Validation` | UNCOVERED |

#### 18. WebConfigError (shared/provider-contracts/src/web_config/error.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Io` | UNCOVERED |
| `Parse` | UNCOVERED |
| `MissingField` | UNCOVERED |
| `InvalidValue` | UNCOVERED |
| `PathNotFound` | UNCOVERED |

#### 19. PublishError (app/generator/src/error.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `MissingField` | UNCOVERED |
| `TemplateNotFound` | UNCOVERED |
| `ProviderFailed` | UNCOVERED |
| `RenderFailed` | UNCOVERED |
| `FetchFailed` | UNCOVERED |
| `Config` | UNCOVERED |
| `PagePrerendererFailed` | UNCOVERED |

### PRIORITY 4: Provider Trait Errors (all 0% coverage)

#### 20. RepositoryError -- traits (shared/traits/src/repository.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |
| `NotFound` | UNCOVERED |
| `Serialization` | UNCOVERED |
| `InvalidData` | UNCOVERED |
| `ConstraintViolation` | UNCOVERED |
| `Other` | UNCOVERED |

#### 21. RegistryError (shared/traits/src/registry.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotFound` | UNCOVERED |
| `Unavailable` | UNCOVERED |
| `Configuration` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 22. JwtProviderError (shared/traits/src/jwt.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `InvalidToken` | UNCOVERED |
| `TokenExpired` | UNCOVERED |
| `MissingAudience` | UNCOVERED |
| `ConfigurationError` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 23. DomainConfigError (shared/traits/src/domain_config.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `LoadError` | UNCOVERED |
| `NotFound` | UNCOVERED |
| `ParseError` | UNCOVERED |
| `ValidationError` | UNCOVERED |
| `Other` | UNCOVERED |

#### 24. FileUploadProviderError (shared/traits/src/file_upload.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `Disabled` | UNCOVERED |
| `ValidationFailed` | UNCOVERED |
| `StorageError` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 25. AnalyticsProviderError (shared/traits/src/analytics.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `SessionNotFound` | UNCOVERED |
| `FingerprintNotFound` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 26. McpServiceProviderError (shared/traits/src/mcp_service.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `ServerNotFound` | UNCOVERED |
| `RegistryUnavailable` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 27. ContextProviderError (shared/traits/src/context_provider.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotFound` | UNCOVERED |
| `AccessDenied` | UNCOVERED |
| `Database` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 28. SessionAnalyticsProviderError (shared/traits/src/session_analytics.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `SessionNotFound` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 29. AiProviderError (shared/traits/src/ai_providers.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `FileNotFound` | UNCOVERED |
| `SessionNotFound` | UNCOVERED |
| `StorageError` | UNCOVERED |
| `ConfigurationError` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 30. ProcessProviderError (shared/traits/src/process.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `NotFound` | UNCOVERED |
| `OperationFailed` | UNCOVERED |
| `PortTimeout` | UNCOVERED |
| `Internal` | UNCOVERED |

#### 31. SchedulerError -- traits (shared/traits/src/scheduler.rs) -- 0% covered

| Variant | Status |
|---------|--------|
| `JobNotFound` | UNCOVERED |
| `Unavailable` | UNCOVERED |
| `ExecutionFailed` | UNCOVERED |
| `Internal` | UNCOVERED |

### PRIORITY 5: Partially Covered Enums (gaps only)

#### 32. CloudError -- 87% covered (2 uncovered)

| Variant | Status |
|---------|--------|
| `ApiError` | UNCOVERED |
| `Network` | UNCOVERED |
| `Io` | UNCOVERED |

Note: `ApiError` is the most critical gap here as it covers all cloud API failures.

#### 33. AiError -- 95% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `SerializationError` | UNCOVERED |

#### 34. UserError -- 86% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |

#### 35. ContentError -- 70% covered (3 uncovered)

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |
| `Serialization` | UNCOVERED |
| `Io` | UNCOVERED |
| `Yaml` | UNCOVERED |

#### 36. AnalyticsError -- 86% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |

#### 37. SyncError -- 67% covered (4 uncovered)

| Variant | Status |
|---------|--------|
| `Io` | UNCOVERED |
| `Http` | UNCOVERED |
| `Json` | UNCOVERED |
| `Database` | UNCOVERED |
| `StripPrefix` | UNCOVERED |
| `Zip` | UNCOVERED |

#### 38. LoggingError -- 88% covered (2 uncovered)

| Variant | Status |
|---------|--------|
| `DatabaseError` | UNCOVERED |
| `QueryError` | UNCOVERED |

#### 39. IntegrationError -- 82% covered (2 uncovered)

| Variant | Status |
|---------|--------|
| `Repository` | UNCOVERED |
| `Serialization` | UNCOVERED |
| `Http` | UNCOVERED |

#### 40. TaskError -- 92% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |

#### 41. ContextError -- 83% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `Database` | UNCOVERED |
| `RoleSerialization` | UNCOVERED |

#### 42. ArtifactError -- 88% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `InvalidSchema` | UNCOVERED |
| `InvalidMetadata` | UNCOVERED |

#### 43. ProtocolError -- 88% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `JsonParse` | UNCOVERED |
| `Database` | UNCOVERED |

#### 44. QueryExecutorError -- 50% covered (1 uncovered)

| Variant | Status |
|---------|--------|
| `ExecutionFailed` | UNCOVERED |

---

## Priority Recommendations

### Critical (security/auth surface -- test immediately)

1. **AuthError** (12 uncovered): Core authentication variants like `InvalidTokenFormat`, `TokenExpired`, `InvalidSignature`, `InsufficientPermissions` have zero test coverage. These are the front door to the application.
2. **TokenError** (9 uncovered): The entire OAuth token endpoint error enum is untested. Covers grant validation, client auth, and code expiry.
3. **ContextExtractionError** (11 uncovered): Request context extraction (JWT parsing, header validation, user lookup) is completely untested. This is the middleware that gates every authenticated route.
4. **CoreError** (22 uncovered): The central error type with `status_code()`, `is_auth_error()`, `is_not_found()` helper methods has zero coverage.

### High (API error mapping and infrastructure)

5. **InternalApiError** (12 uncovered): Maps domain errors to HTTP responses. Untested means the error-to-status-code contract is unverified.
6. **ServiceError** (8 uncovered): The bridge between domain services and API errors. The `From<ServiceError> for ApiError` conversion is untested.
7. **ProfileBootstrapError / SecretsBootstrapError / CredentialsBootstrapError** (20 uncovered total): The entire bootstrap chain has no error path tests. Misconfiguration during startup would be invisible.
8. **PathError** (6 uncovered): File system path validation errors. Affects binary discovery and path resolution.

### Medium (protocol and domain logic)

9. **OAuthParseError** (7 uncovered): String-to-enum parsing for OAuth grant types, PKCE methods, response types. Invalid input handling untested.
10. **A2aParseError** (2 uncovered): A2A protocol request parsing. Malformed JSON-RPC requests would be unhandled.
11. **DatabaseSessionManagerError** (5 uncovered): MCP session lifecycle errors (not found, expired, reconnect).
12. **PublishError** (7 uncovered): Static site generation errors. Rich diagnostic fields (suggestions, source paths) are unverified.
13. **IdValidationError** (2 uncovered): Foundation-layer ID validation. Empty/invalid IDs should be caught early.

### Lower (provider traits -- typically tested via implementations)

14. **Provider trait errors** (12 enums, ~48 variants): `JwtProviderError`, `DomainConfigError`, `FileUploadProviderError`, `AnalyticsProviderError`, `McpServiceProviderError`, `ContextProviderError`, `SessionAnalyticsProviderError`, `AiProviderError`, `ProcessProviderError`, `SchedulerError` (traits), `RegistryError`, `RepositoryError` (traits). These define contracts; implementations may test them indirectly, but the enums themselves (Display, From conversions) are unverified.
