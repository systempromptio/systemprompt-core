# Import/Export Issues

Errors related to module imports, exports, and unused declarations.

## Categories

### 1. Missing Module Imports (TS2307)
Module not found errors.

### 2. Re-export Type Issues (TS1205)
With `verbatimModuleSyntax: true`, type re-exports need `export type`.

### 3. Unused Imports/Declarations (TS6133)
Declared but never used.

---

## Files to Fix

### src/components/artifacts/ArtifactShowcase.tsx
- **Line 3**: `Cannot find module '@/__tests__/fixtures/mockData'`
  - This file imports test fixtures in production code
  - **Fix:** Remove the import or create the mock data file, or make this a test-only component

### src/constants/index.ts
- **Line 9**: Re-exporting type needs `export type`
- **Line 11**: Re-exporting type needs `export type`

**Fix:**
```typescript
// Before
export { SomeType } from './types'

// After
export type { SomeType } from './types'
```

### src/components/chat/hooks/useChatSender.ts
- **Line 26**: `createContextId` declared but never used
  - **Fix:** Remove the unused import

### src/components/tools/ToolResultModal.tsx
- **Line 12**: `createArtifactId` declared but never used
  - **Fix:** Either use it (for the Record indexing fix) or remove it

### src/stores/context.store.ts
- **Line 23**: `AgentUrl` declared but never used
  - **Fix:** Remove from import if not needed after other fixes
