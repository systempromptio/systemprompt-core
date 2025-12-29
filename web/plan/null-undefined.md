# Null vs Undefined Mismatches

Errors where `null` is used but `undefined` is expected, or vice versa.

## Background

The codebase should standardize on `undefined` for optional values (TypeScript convention). Some code uses `null` which causes type mismatches.

## Fix Pattern

```typescript
// Before
setEphemeralArtifact(null)
const value: string | null = undefined  // error

// After
setEphemeralArtifact(undefined)
const value: string | undefined = undefined  // ok
```

For interfaces that expect `null`, change to accept `undefined` instead, or use nullish values consistently.

---

## Files to Fix

### src/components/artifacts/ArtifactModal.tsx
- **Line 22** (FIXED): `setEphemeralArtifact(null)` → `setEphemeralArtifact(undefined)`

### src/components/auth/RegisterModal.tsx
- **Line 41**: `string | undefined` not assignable to `string | null`
  - Change the interface/prop type to accept `undefined` instead of `null`

### src/components/chat/ContextSelector.tsx
- **Line 130**: `string | undefined` not assignable to `string | null`
  - Change the variable/prop type

### src/components/conversations/ConversationToggle.tsx
- **Line 103**: `Conversation | undefined` not assignable to `Conversation | null`
- **Line 113**: `string | undefined` not assignable to `string | null`
  - Update state types to use `undefined`

### src/components/tools/ToolResultModal.tsx
- **Line 69**: `setEphemeralArtifact(null)` → `setEphemeralArtifact(undefined)`

### src/hooks/a2a/useA2AInitializer.ts
- **Line 44**: `string | null` not assignable to `string | undefined`
  - Use `?? undefined` to convert null to undefined
- **Line 87**: `AgentCard | null` not assignable to `AgentCard | undefined`
- **Line 94**: `AgentCard | undefined` not assignable to `AgentCard | null`
  - Standardize on one or the other

### src/hooks/a2a/useA2ATokenRefresh.ts
- **Line 25**: `string | undefined` not assignable to `string | null`

### src/hooks/useAgentDiscovery.ts
- **Line 98**: `null` not assignable to `string | undefined`
- **Line 168**: `null` not assignable to `string | undefined`
  - Change `null` to `undefined`

### src/hooks/useMcpRegistry.ts
- **Line 88**: `null` not assignable to `string | undefined`

### src/hooks/useSSEConnection.ts
- **Line 152**: `null` not assignable to `string | undefined`

### src/hooks/useStreamEventProcessor.ts
- **Line 63**: `null` not assignable to `TaskId | undefined`

### src/stores/auth.store.ts
- **Line 101, 118**: `string | null` not assignable to `string | undefined`
  - The `extractUsernameFromJWT` and `extractUserTypeFromJWT` functions return `null`
  - Either change those functions to return `undefined`, or change the store type to accept `null`
