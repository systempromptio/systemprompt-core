# Object Property Mismatches

Errors where objects have properties that don't exist on their types, or are missing required properties.

---

## Files to Fix

### src/lib/mcp/validator.ts
- **Lines 22, 34, 47, 54**: Object literal has `expected` property but `ValidationError` type doesn't have it

**Current ValidationError type:**
```typescript
type ValidationError = {
  readonly kind: 'validation'
  readonly field: string
  readonly path: readonly string[]
  readonly message: string
}
```

**Fix Options:**

1. **Add `expected` to ValidationError type** (in `src/types/core/errors.ts`):
```typescript
type ValidationError = {
  readonly kind: 'validation'
  readonly field: string
  readonly path: readonly string[]
  readonly message: string
  readonly expected?: string  // Add this
}
```

2. **Remove `expected` from validator.ts** and include it in the message instead:
```typescript
// Before
return {
  kind: 'validation',
  field: 'type',
  path: ['type'],
  message: 'Invalid type',
  expected: 'string'  // Error: doesn't exist
}

// After
return {
  kind: 'validation',
  field: 'type',
  path: ['type'],
  message: 'Invalid type. Expected: string'
}
```

### src/services/webauthn.service.ts
- **Line 190**: `user_id` does not exist on type `{}`
- **Line 258**: Type `{}` missing properties `user_id`, `success`

**Fix:** Define proper response types:
```typescript
interface WebAuthnFinishResponse {
  user_id: string
  success: boolean
}

// Then use it in the fetch response handling
const data: WebAuthnFinishResponse = await response.json()
```

### src/stores/artifact.store.ts
- **Line 187**: Property `kind` does not exist on type `never`
  - This usually indicates a type narrowing issue where TypeScript thinks the value can never exist
  - **Fix:** Check the conditional logic leading to this line - likely need to fix the type guards

### src/hooks/useDeepLink.ts
- **Line 8**: Property `token` does not exist on type returned by `useAuth()`
  - **Fix:** Either add `token` to the useAuth return type, or use a different method to get the token

### src/types/agui/guards.ts
- **Line 27**: String not assignable to event type union
  - The type guard is checking if a string is one of the valid event types
  - **Fix:** Use `as const` assertion or adjust the type guard:
```typescript
// Before
const validTypes = ['RUN_STARTED', 'RUN_FINISHED', ...]
return validTypes.includes(event.type)

// After
const validTypes = ['RUN_STARTED', 'RUN_FINISHED', ...] as const
type ValidType = typeof validTypes[number]
return validTypes.includes(event.type as ValidType)
```

### src/hooks/useStreamEventProcessor.ts
- **Line 87**: `unknown[]` not assignable to `ContextSnapshotItem[]`
  - **Fix:** Add type assertion or validation:
```typescript
// Option 1: Type assertion (less safe)
handleSnapshot(contexts as ContextSnapshotItem[])

// Option 2: Runtime validation (safer)
if (isContextSnapshotArray(contexts)) {
  handleSnapshot(contexts)
}
```
