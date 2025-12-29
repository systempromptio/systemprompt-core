# Record Indexing with String Errors

Errors where a plain `string` is used to index a `Record<BrandedType, Value>` type, causing implicit `any`.

## Background

When a Record uses a branded type as key (e.g., `Record<TaskId, Task>`), you cannot index it with a plain string. You must either:
1. Cast the string to the branded type first
2. Use type assertion
3. Add proper type guards

## Fix Pattern

```typescript
// Before - ERROR
const task = byId[taskId]  // taskId is string, byId is Record<TaskId, Task>

// After - Option 1: Cast with factory function
const task = byId[createTaskId(taskId)]

// After - Option 2: Type assertion (less safe)
const task = byId[taskId as TaskId]

// After - Option 3: Use Object methods with type guard
const task = Object.entries(byId).find(([id]) => id === taskId)?.[1]
```

---

## Files to Fix

### src/components/chat/TaskList.tsx
- **Line 32**: `stepsByTask[task.id]` where `task.id` is string, `stepsByTask` is `Record<TaskId, ExecutionId[]>`
- **Line 54**: Same issue

### src/components/chat/hooks/useChatSender.ts
- **Line 72**: `byId[event.taskId]` - cast `event.taskId` with `createTaskId()`

### src/components/chat/hooks/useTaskLoader.ts
- **Line 33**: `taskIdsByContext[currentContextId]` - guard against LOADING state first

### src/components/chat/metadata/TaskExecutionSteps.tsx
- **Line 19**: `stepsByTask[taskId]` - cast with `createTaskId()`

### src/components/tools/ToolResultModal.tsx
- **Line 53-54**: `artifactsById[artifactId]` - cast with `createArtifactId()`

### src/hooks/useDeepLink.ts
- **Line 22**: `artifactsById[artifactId]` - cast with `createArtifactId()`

### src/hooks/useStreamEventProcessor.ts
- **Line 43**: `byId[event.id]` - cast with `createTaskId()`

### src/stores/ui-state.store.ts
- **Line 278**: `toolExecutionsById[executionId]` - cast with `createExecutionId()`
- **Line 297**: `stepsByTask[taskId]` - cast with `createTaskId()` (appears twice)
