# Service Error Type Mismatches

Errors where services return `ApiError` from api-client but function signatures expect domain-specific error types like `ArtifactError` or `TaskError`.

## Background

The api-client returns `AsyncResult<T, ApiError>` where `ApiError` is:
```typescript
type ApiError =
  | NetworkError
  | TimeoutError
  | ParseError
  | UnauthorizedError
  | ForbiddenError
  | RateLimitError
  | ServerError
  | UnknownError
```

But service files declare return types with domain-specific errors like:
```typescript
type ArtifactError = NotFoundError | ValidationError | NetworkError
type TaskError = NotFoundError | ValidationError | NetworkError
```

These types don't overlap completely, causing assignment errors.

## Fix Pattern

**Option 1: Use ApiError directly (Recommended)**
Change service return types to use `ApiError`:

```typescript
// Before
async function getArtifacts(): AsyncResult<Artifact[], ArtifactError>

// After
async function getArtifacts(): AsyncResult<Artifact[], ApiError>
```

**Option 2: Map errors**
Create a mapping function to convert `ApiError` to domain-specific errors:

```typescript
function toArtifactError(error: ApiError): ArtifactError {
  if (error.kind === 'network') return error
  // ... map other cases
  return createNetworkError(0, 'Unknown error')
}
```

---

## Files to Fix

### src/services/artifacts.service.ts
- **Line 38**: Return type mismatch - change `ArtifactError` to `ApiError`
- **Line 55**: Same issue
- **Line 72**: Same issue
- **Line 92**: Same issue

**Fix:** Change all `AsyncResult<..., ArtifactError>` to `AsyncResult<..., ApiError>`

```typescript
// Change import
import type { AsyncResult, ApiError } from '@/types/core'

// Change function signatures
async listArtifacts(...): AsyncResult<readonly Artifact[], ApiError>
async getArtifactsByContext(...): AsyncResult<readonly Artifact[], ApiError>
async getArtifactsByTask(...): AsyncResult<readonly Artifact[], ApiError>
async getArtifact(...): AsyncResult<Artifact, ApiError>
```

### src/services/tasks.service.ts
- **Line 18**: Return type mismatch - change `TaskError` to `ApiError`
- **Line 35**: Same issue
- **Line 54**: Same issue

**Fix:** Change all `AsyncResult<..., TaskError>` to `AsyncResult<..., ApiError>`

```typescript
// Change import
import type { AsyncResult, ApiError } from '@/types/core'

// Change function signatures
async listTasks(...): AsyncResult<readonly Task[], ApiError>
async getTask(...): AsyncResult<Task, ApiError>
async getTasksByContext(...): AsyncResult<readonly Task[], ApiError>
```

## Note

After these changes, consumers of these services will receive `ApiError` instead of domain-specific errors. Update error handling code accordingly - the `error.kind` discriminant can still be used to handle specific error types.
