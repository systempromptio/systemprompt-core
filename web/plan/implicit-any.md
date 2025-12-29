# Implicit Any Parameter Errors

Errors where function parameters implicitly have `any` type due to missing type annotations.

## Background

With `strict: true` in tsconfig, TypeScript requires explicit type annotations for parameters when it cannot infer the type. This commonly happens in:
- Array method callbacks (`.map()`, `.filter()`, `.find()`, `.sort()`)
- Object method callbacks (`Object.entries()`, `Object.keys()`)

## Fix Pattern

```typescript
// Before - ERROR
.map((id) => ...)  // Parameter 'id' implicitly has an 'any' type
.sort((a, b) => ...)  // Parameters 'a' and 'b' implicitly have an 'any' type

// After - Add explicit types
.map((id: TaskId) => ...)
.sort((a: ExecutionStep, b: ExecutionStep) => ...)

// Or use type inference from the array
const steps: ExecutionStep[] = ...
steps.map((step) => ...)  // 'step' is inferred as ExecutionStep
```

---

## Files to Fix

### src/components/chat/TaskList.tsx
- **Line 56**: `(id)` → `(id: ExecutionId)`
- **Line 57**: `(step)` → `(step: ExecutionStep)`
- **Line 58**: `(a, b)` → `(a: ExecutionStep, b: ExecutionStep)`

### src/components/chat/hooks/useTaskLoader.ts
- **Line 35**: `(id)` → `(id: TaskId)`
- **Line 36**: `(task)` → `(task: Task)`
- **Line 37**: `(task)` → `(task: Task)`
- **Line 38**: `(a, b)` → `(a: Task, b: Task)`

### src/components/chat/metadata/TaskExecutionSteps.tsx
- **Line 21**: `(id)` → `(id: ExecutionId)`
- **Line 22**: `(step)` → `(step: ExecutionStep)`
- **Line 23**: `(a, b)` → `(a: ExecutionStep, b: ExecutionStep)`

### src/stores/ui-state.store.ts
- **Line 298**: `(id)` → `(id: ExecutionId)`

## Alternative Approach

Instead of annotating each callback parameter, you can type the intermediate array:

```typescript
// Before
const stepIds = stepsByTask[taskId] || []
stepIds.map((id) => ...)  // error: 'id' is any

// After - Type the array explicitly
const stepIds: readonly ExecutionId[] = stepsByTask[createTaskId(taskId)] || []
stepIds.map((id) => ...)  // 'id' is now ExecutionId
```

This approach also solves the Record indexing issue at the same time.
