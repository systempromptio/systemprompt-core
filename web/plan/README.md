# TypeScript Error Fix Plans

This directory contains categorized plans for fixing all TypeScript errors in the codebase.

## Error Summary

| Category | Count | Plan File |
|----------|-------|-----------|
| Branded Type Errors | ~35 | [branded-types.md](./branded-types.md) |
| Null vs Undefined | ~15 | [null-undefined.md](./null-undefined.md) |
| Record Indexing | ~12 | [record-indexing.md](./record-indexing.md) |
| Implicit Any | ~15 | [implicit-any.md](./implicit-any.md) |
| Service Error Types | ~7 | [service-errors.md](./service-errors.md) |
| Import/Export Issues | ~5 | [import-export.md](./import-export.md) |
| Property Mismatches | ~10 | [property-mismatches.md](./property-mismatches.md) |

## Recommended Fix Order

1. **[service-errors.md](./service-errors.md)** - Quick wins, just change return types
2. **[import-export.md](./import-export.md)** - Quick fixes for module issues
3. **[null-undefined.md](./null-undefined.md)** - Standardize on `undefined`
4. **[branded-types.md](./branded-types.md)** - Largest category, systematic
5. **[record-indexing.md](./record-indexing.md)** - Related to branded types
6. **[implicit-any.md](./implicit-any.md)** - Add type annotations
7. **[property-mismatches.md](./property-mismatches.md)** - Requires more investigation

## Files with Most Errors

| File | Error Count | Main Issue |
|------|-------------|------------|
| `src/components/chat/hooks/useTaskLoader.ts` | 9 | Branded types, implicit any |
| `src/components/chat/TaskList.tsx` | 6 | Record indexing, implicit any |
| `src/hooks/useAgentDiscovery.ts` | 5 | Branded types, null/undefined |
| `src/components/conversations/ConversationToggle.tsx` | 5 | Branded types, null/undefined |
| `src/stores/ui-state.store.ts` | 4 | Record indexing, implicit any |
| `src/services/artifacts.service.ts` | 4 | Service error types |
| `src/lib/mcp/validator.ts` | 4 | Property mismatches |
| `src/hooks/useDeepLink.ts` | 5 | Branded types, property access |

## Running Type Check

```bash
npx tsc --project tsconfig.app.json --noEmit
```

## Already Fixed Files

Some files have already been partially fixed:
- `src/services/auth.service.ts` - Result type handling
- `src/services/contexts.service.ts` - Error type alignment
- `src/stores/context.store.ts` - Branded types (partial)
- `src/hooks/agui/useAgUiEventProcessor.ts` - Branded types (partial)
- `src/components/agents/AgentSelector.tsx` - AgentUrl
- `src/components/agents/AgentSelectorMobile.tsx` - AgentUrl
- `src/components/artifacts/ArtifactModal.tsx` - null → undefined
- `src/components/chat/hooks/useChatSender.ts` - null → undefined (partial)
