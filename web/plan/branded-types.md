# Branded Type Errors

Errors where plain `string` is passed instead of branded types like `ContextId`, `TaskId`, `AgentUrl`, `ArtifactId`, or `ExecutionId`.

## Background

The codebase uses TypeScript branded types for type safety:
```typescript
type ContextId = string & { readonly __brand: "ContextId" }
type TaskId = string & { readonly __brand: "TaskId" }
type AgentUrl = string & { readonly __brand: "AgentUrl" }
type ArtifactId = string & { readonly __brand: "ArtifactId" }
type ExecutionId = string & { readonly __brand: "ExecutionId" }
```

Factory functions exist in `@/types/core/brand`:
- `createContextId(id: string): ContextId`
- `createTaskId(id: string): TaskId`
- `createAgentUrl(url: string): AgentUrl`
- `createArtifactId(id: string): ArtifactId`
- `createExecutionId(id: string): ExecutionId`

## Fix Pattern

1. Import the appropriate `create*` function
2. Wrap string values with the factory function

```typescript
// Before
selectAgent(agent.url, agent)

// After
import { createAgentUrl } from '@/types/core/brand'
selectAgent(createAgentUrl(agent.url), agent)
```

---

## Files to Fix

### src/App.tsx
- **Line 84**: `currentContextId` passed to `contextAgents.get()` - guard against LOADING state
- **Line 92**: `matchingAgent.url` → `createAgentUrl(matchingAgent.url)`

### src/components/artifacts/ArtifactsView.tsx
- **Line 70**: string → `createContextId()`

### src/components/chat/ContextSelector.tsx
- **Line 72**: `conversation.id` → already ContextId, check usage
- **Line 85**: string → `createContextId()`
- **Line 96**: string → `createContextId()`

### src/components/chat/TaskView.tsx
- **Line 47**: string → `createContextId()`
- **Line 71**: `string[]` → `ArtifactId[]` - map with `createArtifactId()`

### src/components/chat/hooks/useArtifactAccumulator.ts
- **Line 66**: string → `createTaskId()`

### src/components/chat/hooks/useChatSender.ts
- **Line 128**: string → `createTaskId()`

### src/components/chat/hooks/useTaskLoader.ts
- **Line 57**: guard against LOADING state before passing to function
- **Line 98**: taskId in object → `createTaskId()`
- **Line 107**: string → `createTaskId()`

### src/components/conversations/ConversationToggle.tsx
- **Line 61**: string → `createContextId()`
- **Line 77**: string → `createContextId()`
- **Line 87**: string → `createContextId()`

### src/hooks/a2a/useA2AInitializer.ts
- **Line 58**: string → `createAgentUrl()`

### src/hooks/useA2AClientInitialization.ts
- **Line 52**: string → `createAgentUrl()`

### src/hooks/useAgentDiscovery.ts
- **Line 121**: guard against LOADING state
- **Line 129**: string → `createAgentUrl()`
- **Line 141**: string → `createAgentUrl()`

### src/hooks/useDeepLink.ts
- **Line 23**: string → `createArtifactId()`
- **Line 26**: string → `createArtifactId()`
- **Line 27**: string → `createArtifactId()`

### src/hooks/useMcpToolCaller.ts
- **Line 101**: wrong type used (ContextId for TaskId)
- **Line 183**: uuid string → `createExecutionId()`
- **Line 201**: uuid string → `createExecutionId()`

### src/hooks/useSSEConnection.ts
- **Line 104**: guard against LOADING state

### src/hooks/useStreamEventProcessor.ts
- **Line 62**: string → `createTaskId()`

### src/types/agui/guards.ts
- **Line 27**: string not assignable to event type union - fix type guard
