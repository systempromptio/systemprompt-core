# Web Codebase Refactor Plan

**Objective:** Refactor `/var/www/html/systemprompt-core/web/src` to comply with SystemPrompt TypeScript Standards defined in `/instructions/typescript.md` and `/instructions/typescript-review.md`.

**Current State:** 288 TypeScript files, 34,655 lines of code with significant violations.

**Exceptions:** Service classes (singleton pattern) will be preserved.

---

## CRITICAL: No Fuzzy Data Policy (NEW)

**Standards updated 2024-12-19:** Functions MUST NOT return `undefined` to indicate failure.

- Functions either return the value, return `Result<T, E>`, or throw
- No `T | undefined` return types
- No `return undefined` on failure
- All failures must be explicit in return type

Previous work replacing `null` with `undefined` needs revision to use Result pattern.

---

## Progress Summary

### Wave 1: Foundation - COMPLETED
- [x] Created `src/types/core/` with Result, Option, Brand, AsyncState types
- [x] Created `src/constants/breakpoints.ts` and `src/constants/retry.ts`
- [x] Fixed 13 forbidden abbreviations (err→error, obj→record, val→value, msg→message)
- [x] Deleted commented-out TasksView code in ViewRouter.tsx
- [x] Consolidated ArtifactType definitions (types/artifact.ts is canonical)
- [x] Consolidated ValidationError definitions (types/core/errors.ts is canonical)
- [x] Deleted unused retry-strategy.ts
- [x] Fixed 4 `any` types in webauthn.service.ts
- [x] Fixed non-null assertion in main.tsx
- [x] Renamed task-icon.tsx to TaskIcon.tsx (PascalCase)

### Wave 2: Store Refactoring - COMPLETED ✓
- [x] Replaced null with undefined in stores
- [x] Stores now consume Result types from services
- [x] Error handling properly propagates from services

### Wave 3: File Splitting - COMPLETED ✓
- [x] Split ExecutionTimeline.tsx (659 lines → 5 files)

### Wave 4-5: Null→Undefined Work - COMPLETED ✓
- [x] Replaced null with undefined across codebase
- [x] Services now return Result types (Wave 6)

### File Size Status (500 line limit) - ALL COMPLIANT ✓
All files are under 500 lines. No splitting required.

### Infrastructure in Place ✓
- [x] Branded types defined in `src/types/core/brand.ts`
- [x] Result pattern defined in `src/types/core/result.ts`
- [x] AsyncResult type for async operations
- [x] Domain error types: ApiError, TaskError, ArtifactError, ContextError

---

## Wave 6: No Fuzzy Data Implementation - COMPLETED ✓

All functions that can fail now return `Result<T, E>` or throw.

### 6.1 Service Layer - COMPLETED ✓

**Updated files:**
- `services/api-client.ts` - Returns `AsyncResult<T, ApiError>` with Ok/Err constructors
- `services/tasks.service.ts` - Returns `AsyncResult<T, TaskError>`
- `services/artifacts.service.ts` - Returns `AsyncResult<T, ArtifactError>`
- `services/contexts.service.ts` - Returns `AsyncResult<T, ContextError>`

### 6.2 Store Layer - COMPLETED ✓

**Updated files:**
- `stores/task.store.ts` - Handles Result types from services
- `stores/artifact.store.ts` - Handles Result types from services
- `stores/context.store.ts` - Handles Result types from services

Stores maintain error state for UI display but consume proper Result types.

### 6.3 Hook Layer - COMPLETED ✓

**Updated files:**
- `hooks/useMcpToolCaller.ts` - constructEphemeralArtifact throws on invalid
- `hooks/useDynamicOptions.ts` - fetchFullObject throws on failure
- `hooks/useA2AMessageOperations.ts` - All methods throw on failure
- `hooks/a2a/useA2AMessaging.ts` - All methods throw on failure

### 6.4 A2A Client - COMPLETED ✓

**Updated `lib/a2a/client.ts`:**
- `getTask()` - Returns `Promise<Task>`, throws on failure
- `cancelTask()` - Returns `Promise<Task>`, throws on failure
- `sendMessage()` - Returns `Promise<Task | Message>`, throws on failure
- `getAgentCard()` - Returns `AgentCard`, throws if not initialized
- Added `hasAgentCard()` for checking without throwing

### 6.5 Type Infrastructure - COMPLETED ✓

Core types in `types/core/`:
- `Result<T, E>` discriminated union
- `AsyncResult<T, E>` = `Promise<Result<T, E>>`
- `Ok(value)` and `Err(error)` constructors
- Error factory functions: `createNotFoundError`, `createNetworkError`, etc.

---

## Deferred Work (Future Enhancements) - COMPLETED ✓

- [x] Adopt branded types in stores (context.store, artifact.store, task.store, agent.store, ui-state.store)
- [x] Fix module boundary violations (useTaskLoader.ts now uses store instead of service)
- [x] Updated ExecutionStep type to use branded types (ExecutionId, TaskId, SkillId)

---

## Phase 1: Foundation - Core Types & Patterns

### 1.1 Create Core Type Infrastructure

**Create:** `src/types/core/result.ts`
```typescript
type Result<T, E = Error> = { ok: true; value: T } | { ok: false; error: E };
type AsyncResult<T, E = Error> = Promise<Result<T, E>>;
const Ok = <T>(value: T): Result<T, never> => ({ ok: true, value });
const Err = <E>(error: E): Result<never, E> => ({ ok: false, error });
```

**Create:** `src/types/core/option.ts`
```typescript
type Option<T> = { some: true; value: T } | { some: false };
const Some = <T>(value: T): Option<T> => ({ some: true, value });
const None: Option<never> = { some: false };
```

**Create:** `src/types/core/brand.ts`
```typescript
type Brand<T, B> = T & { readonly __brand: B };
type UserId = Brand<string, "UserId">;
type ContextId = Brand<string, "ContextId">;
type TaskId = Brand<string, "TaskId">;
type ArtifactId = Brand<string, "ArtifactId">;
type AgentUrl = Brand<string, "AgentUrl">;
type AuthToken = Brand<string, "AuthToken">;
// Factory functions for each
```

**Create:** `src/types/core/index.ts` - Re-export all core types

---

## Phase 2: Eliminate Duplicates

### 2.1 Consolidate ArtifactType (CRITICAL - Incompatible definitions)

**Files:**
- `src/types/artifact.ts:7-20`
- `src/constants/artifacts.ts:4-18`

**Action:** Keep single definition in `src/types/artifact.ts`, update `constants/artifacts.ts` to re-export.

### 2.2 Consolidate ValidationError

**Files:**
- `src/lib/mcp/types.ts:61-66` - `{ path: string[], message: string }`
- `src/lib/schema/types.ts:52-55` - `{ field: string, message: string }`

**Action:** Create unified `src/types/core/errors.ts`:
```typescript
type ValidationError = {
  readonly kind: "validation";
  readonly field: string;
  readonly path: readonly string[];
  readonly message: string;
};
```

### 2.3 Consolidate Type Guards (CRITICAL - Different logic for same concept)

**Files:**
- `src/types/artifact.ts:265-270` - Original
- `src/utils/type-guards.ts:30-62` - Duplicate with different logic

**Action:**
1. Keep canonical guards in `src/types/artifact.ts`
2. Split `src/utils/type-guards.ts` (495 lines) into:
   - `src/types/artifact-guards.ts`
   - `src/types/task-guards.ts`
   - `src/types/agui-guards.ts`
3. Delete duplicates from utils

### 2.4 Consolidate Retry Logic (3 implementations)

**Files:**
- `src/utils/fetch-with-retry.ts` - Keep (utility)
- `src/hooks/useRetry.ts` - Keep (React hook)
- `src/utils/retry-strategy.ts` - DELETE (redundant)

**Action:** Standardize config, create shared `src/constants/retry.ts`:
```typescript
const RETRY_CONFIG = {
  maxAttempts: 3,
  baseDelayMs: 1000,
  maxDelayMs: 10000,
  retryableStatuses: [429, 500, 502, 503, 504],
} as const;
```

### 2.5 Consolidate Async State Types

**Files:**
- `src/hooks/useAsyncState.ts:11-15`
- `src/types/hooks.ts:25-40, 54-69, 124-144`

**Action:** Create single `src/types/async-state.ts`:
```typescript
type AsyncState<T, E = Error> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; data: T }
  | { status: "error"; error: E };
```

---

## Phase 3: Remove Forbidden Constructs

### 3.1 Eliminate `any` Types (4 violations)

**File:** `src/services/webauthn.service.ts`
- Line 4: `data?: any` → Create `WebAuthnRequestData` type
- Line 29, 34: `publicKey: any` → Use `PublicKeyCredentialCreationOptions`
- Line 97: `body?: any` → Use `WebAuthnRequestBody` union type

### 3.2 Reduce `as` Type Assertions (90+ violations)

**Priority files:**
- `src/types/artifact.ts:278-432` - Replace with proper type guards
- `src/types/execution.ts:60-123` - Use discriminated unions
- `src/hooks/useMcpToolCaller.ts:26-256` - Add runtime type checking
- `src/services/api-client.ts:237-330` - Use Result pattern

### 3.3 Fix Non-null Assertion

**File:** `src/main.tsx:15`
```typescript
// Before
createRoot(document.getElementById('root')!).render(...)

// After
const rootElement = document.getElementById('root');
if (!rootElement) throw new Error('Root element not found');
createRoot(rootElement).render(...)
```

### 3.4 Replace `null` with `undefined` (50+ violations)

**Priority files:**
- `src/contexts/AuthContext.tsx:12-60`
- `src/stores/ui-state.store.ts:32-82`
- `src/stores/agent.store.ts:10-34`
- `src/stores/skill.store.ts:7-20`
- `src/stores/agui.store.ts:20-45`
- `src/services/api-client.ts:41-173`

### 3.5 Eliminate Forbidden Abbreviations (13 violations)

| File | Line | Current | Replace With |
|------|------|---------|--------------|
| `fetch-with-retry.ts` | 27 | `err` | `error` |
| `useA2AMessaging.ts` | 117, 123 | `err` | `error` |
| `useA2AClientInitialization.ts` | 64 | `err` | `error` |
| `useA2AMessageOperations.ts` | 57, 86 | `err` | `error` |
| `type-guards.ts` | 33, 59, 225, 249 | `obj` | `value` or `record` |
| `type-guards.ts` | 426 | `msg` | `message` |
| `artifact-categorization.ts` | 80 | `obj` | `data` |
| `NumberField.tsx` | 41 | `val` | `value` |

---

## Phase 4: Delete Dead Code & Comments

### 4.1 Remove Commented-Out Code

**File:** `src/components/views/ViewRouter.tsx:29-44`
- Delete 15 lines of commented TasksView code

### 4.2 Remove All Inline Comments (308 instances)

**Priority files with most comments:**
- `src/types/artifact.ts` - Section headers
- `src/constants/timing.ts` - Descriptive comments
- `src/lib/schema/` - Explanatory comments

**Approach:** Remove all `//` comments. If logic isn't clear, rename variables/functions.

### 4.3 Remove Obvious JSDoc

Delete JSDoc that just restates function/param names. Keep only for non-obvious business rules.

---

## Phase 5: Fix Structural Violations

### 5.1 Split Oversized Files (15 files > 300 lines)

| File | Lines | Action |
|------|-------|--------|
| `ExecutionTimeline.tsx` | 659 | Split into `StreamingView.tsx`, `StaticView.tsx`, `ModalStepCard.tsx`, `DetailSection.tsx` |
| `context.store.ts` | 520 | Extract `context-actions.ts`, `context-selectors.ts` |
| `type-guards.ts` | 495 | Split by domain (see 2.3) |
| `theme.types.ts` | 491 | Split into `colors.ts`, `typography.ts`, `spacing.ts` |
| `artifact.store.ts` | 450 | Extract `artifact-actions.ts`, `artifact-selectors.ts` |
| `artifact.ts` | 441 | Extract guards to separate file |
| `schema/validator.ts` | 428 | Split by validation type |
| `schema/resolver.ts` | 428 | Split by resolution strategy |
| `api-client.ts` | 407 | Extract `request-builder.ts`, `response-handler.ts` |
| `date-parsing.ts` | 378 | Extract `date-formatters.ts` |
| `ui-state.store.ts` | 378 | Extract actions/selectors |
| `task.store.ts` | 374 | Extract actions/selectors |
| `auth.store.ts` | 371 | Extract actions/selectors |
| `App.tsx` | 371 | Extract route config, provider wrapper |

### 5.2 Split Oversized Functions (8+ violations)

| File | Function | Lines | Action |
|------|----------|-------|--------|
| `ExecutionTimeline.tsx` | `StreamingView` | 159 | Extract sub-components |
| `ExecutionTimeline.tsx` | `ModalStepCard` | 145 | Extract detail renderers |
| `useMcpToolCaller.ts` | `callTool` | 140 | Extract artifact processing, error handling |
| `useTaskLoader.ts` | `useTaskLoader` | 128 | Extract fetch logic, state updates |
| `context.store.ts` | `handleStateEvent` | 99 | Extract event handlers per type |
| `context.store.ts` | `handleSnapshot` | 58 | Extract parsing, state update |
| `context.store.ts` | `createConversation` | 54 | Extract API call, state update |
| `ExpandableToolArgs` | - | 51 | Extract formatter |

### 5.3 Fix File Naming

**File:** `src/components/views/task-icon.tsx`
- Rename to `TaskIcon.tsx` (PascalCase for components)

### 5.4 Fix Module Boundary Violations

**Violation 1:** `src/components/auth/LoginModal.tsx:3`
```typescript
// Before
import { webAuthnService } from '@/services/webauthn.service'

// After - Use store or context
import { useAuthStore } from '@/stores/auth.store'
```

**Violation 2:** `src/components/chat/hooks/useTaskLoader.ts:7`
```typescript
// Before
import { tasksService } from '@/services/tasks.service'

// After - Use store
import { useTaskStore } from '@/stores/task.store'
```

### 5.5 Extract Magic Numbers to Constants

**Create:** `src/constants/breakpoints.ts`
```typescript
const BREAKPOINTS = {
  mobile: 768,
  tablet: 1024,
  desktop: 1280,
} as const;
```

**Files to update:**
- `src/components/ui/hooks/useModalState.ts:14`
- `src/components/layout/AppLayout.tsx:26,31`

---

## Phase 6: Add Branded Types

### 6.1 Update Store Interfaces

**Files:**
- `src/stores/context.store.ts` - Use `ContextId`, `UserId`
- `src/stores/artifact.store.ts` - Use `ArtifactId`, `ContextId`, `TaskId`
- `src/stores/task.store.ts` - Use `TaskId`, `ContextId`
- `src/stores/agent.store.ts` - Use `AgentUrl`
- `src/services/api-client.ts` - Use `AuthToken`

### 6.2 Create Factory Functions

**File:** `src/types/core/brand.ts`
```typescript
const createUserId = (id: string): UserId => id as UserId;
const createContextId = (id: string): ContextId => id as ContextId;
const createTaskId = (id: string): TaskId => id as TaskId;
const createArtifactId = (id: string): ArtifactId => id as ArtifactId;
```

---

## Phase 7: Implement Result Pattern

### 7.1 Convert Service Methods

**Priority services (preserving class structure):**
- `src/services/api-client.ts` - All request methods return `AsyncResult<T, ApiError>`
- `src/services/webauthn.service.ts` - All public methods return `Result<T, WebAuthnError>`
- `src/services/auth.service.ts` - All public methods return `Result<T, AuthError>`
- `src/services/artifacts.service.ts` - All public methods return `Result<T, ArtifactError>`

### 7.2 Replace Try/Catch with Result

**Pattern:**
```typescript
// Before
async fetchUser(id: string): Promise<User> {
  try {
    const response = await fetch(`/api/users/${id}`);
    if (!response.ok) throw new Error('Failed');
    return await response.json();
  } catch (error) {
    throw error;
  }
}

// After
async fetchUser(id: UserId): AsyncResult<User, ApiError> {
  const response = await fetch(`/api/users/${id}`);
  if (!response.ok) {
    return Err({ kind: 'fetch_failed', status: response.status });
  }
  return Ok(await response.json());
}
```

---

## Phase 8: Use Discriminated Unions

### 8.1 Fix API Response States

**Files:**
- `src/stores/ui-state.store.ts` - Use `AsyncState<T>` instead of optional fields
- `src/stores/artifact.store.ts` - Selection state as union

### 8.2 Fix Component Props

**File:** `src/components/chat/ExecutionTimeline.tsx:60-78`
```typescript
// Before
type StreamingProps = { mode: 'streaming'; variant?: 'standalone' | 'bubble'; }
type StaticProps = { mode: 'static'; initialCollapsed?: boolean; }

// After
type ExecutionTimelineProps =
  | { mode: 'streaming'; variant: 'standalone' | 'bubble'; steps: ExecutionStep[] }
  | { mode: 'static'; initialCollapsed: boolean; steps: ExecutionStep[] };
```

---

## Execution Order

### Wave 1: Foundation (No Breaking Changes)
1. Create `src/types/core/` with Result, Option, Brand types
2. Create `src/constants/breakpoints.ts`, `src/constants/retry.ts`
3. Fix forbidden abbreviations (13 renames)
4. Delete commented-out code in ViewRouter.tsx

### Wave 2: Consolidation (Internal Refactoring)
1. Consolidate ArtifactType definitions
2. Consolidate ValidationError definitions
3. Split `type-guards.ts` into domain-specific files
4. Delete `retry-strategy.ts`

### Wave 3: Type Safety (Breaking Changes to Internals)
1. Replace `null` with `undefined` in stores
2. Fix `any` types in webauthn.service.ts
3. Fix non-null assertion in main.tsx
4. Add branded types to stores

### Wave 4: File Structure (File Operations)
1. Split oversized files (ExecutionTimeline.tsx first)
2. Rename task-icon.tsx to TaskIcon.tsx
3. Move misplaced files
4. Remove inline comments

### Wave 5: Architecture (API Changes)
1. Convert services to Result pattern (preserving class structure)
2. Fix module boundary violations
3. Implement discriminated unions for state
4. Split oversized functions

---

## Verification Checklist

After each wave, run:
```bash
npx tsc --noEmit --noUnusedLocals --noUnusedParameters
npx eslint . --max-warnings 0
npx ts-prune
npx jscpd src --threshold 0
npm run build
```

---

## Files to Create

| Path | Purpose |
|------|---------|
| `src/types/core/result.ts` | Result & AsyncResult types |
| `src/types/core/option.ts` | Option type |
| `src/types/core/brand.ts` | Branded types + factories |
| `src/types/core/errors.ts` | Unified error types |
| `src/types/core/index.ts` | Re-exports |
| `src/constants/breakpoints.ts` | Responsive breakpoints |
| `src/constants/retry.ts` | Retry configuration |
| `src/types/artifact-guards.ts` | Artifact type guards |
| `src/types/task-guards.ts` | Task type guards |
| `src/types/agui-guards.ts` | AgUI type guards |

## Files to Delete

| Path | Reason |
|------|--------|
| `src/utils/retry-strategy.ts` | Redundant with fetch-with-retry.ts |
| `src/examples/ThemeExample.tsx` | Example file in src (move to docs) |

## Files to Rename

| Current | New |
|---------|-----|
| `src/components/views/task-icon.tsx` | `src/components/views/TaskIcon.tsx` |

---

## Summary of Violations

| Category | Count | Priority |
|----------|-------|----------|
| `any` types | 4 | HIGH |
| `as` assertions | 90+ | MEDIUM |
| `null` usage | 50+ | MEDIUM |
| Non-null assertions | 1 | MEDIUM |
| Inline comments | 308 | LOW |
| Dead code | 15+ lines | MEDIUM |
| Files > 300 lines | 15 | MEDIUM |
| Functions > 50 lines | 8+ | MEDIUM |
| Module violations | 2 | HIGH |
| Missing branded types | 20+ | HIGH |
| Forbidden abbreviations | 13 | LOW |
| Duplicate types | 5+ | HIGH |
| Duplicate logic | 3+ patterns | HIGH |
