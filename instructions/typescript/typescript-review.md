# SystemPrompt TypeScript Code Review Standards

**Reference:** All TypeScript code must comply with `/instructions/typescript.md`. This review ensures architectural excellence with special focus on eliminating dead code, comments, and duplication.

---

## 1. CRITICAL: Dead Code Elimination

Dead code is the highest priority violation. Every unused line is technical debt.

### Detection Checklist

| Category | How to Detect | Action |
|----------|---------------|--------|
| Unused exports | `tsc --noUnusedLocals` / ESLint `no-unused-vars` | Delete immediately |
| Unused imports | ESLint `no-unused-imports` | Delete immediately |
| Unused functions | IDE "Find References" shows 0 usages | Delete immediately |
| Unused types/interfaces | IDE "Find References" shows 0 usages | Delete immediately |
| Unused parameters | Prefix with `_` only if interface required, else delete |  |
| Unreachable code | After `return`, `throw`, `break`, `continue` | Delete immediately |
| Feature flags never enabled | Search for usage, verify in config | Delete flag and gated code |
| Deprecated code with no callers | Check all references | Delete immediately |

### Common Dead Code Patterns

| Pattern | Example | Action |
|---------|---------|--------|
| Commented-out code | `// const oldImpl = ...` | Delete - git has history |
| "Just in case" exports | Exported but never imported elsewhere | Make private or delete |
| Backup functions | `handleSubmitOld`, `processV1` | Delete the old version |
| Unused enum members | Enum member never matched | Delete member |
| Console.log debugging | `console.log("debug:", x)` | Delete all |
| Empty catch blocks | `catch (e) {}` | Handle error or remove try-catch |
| Placeholder implementations | `function todo() { return undefined; }` | Implement or delete |

### Enforcement Commands

```bash
# Find unused exports
npx ts-prune

# Find unused dependencies
npx depcheck

# ESLint dead code rules
eslint --rule 'no-unused-vars: error' --rule '@typescript-eslint/no-unused-vars: error'

# TypeScript strict checks
tsc --noUnusedLocals --noUnusedParameters
```

**Rule:** If code has no references, it has no purpose. Delete it.

---

## 2. CRITICAL: No Fuzzy Data Policy

Functions NEVER return `undefined` to indicate failure or absence. Every function either:
1. Returns the actual value (success)
2. Returns `Result<T, E>` (expected failure)
3. Throws (programming error/unexpected failure)

### Fuzzy Data Detection

| Pattern | Example | Resolution |
|---------|---------|------------|
| `T \| undefined` return | `function get(): User \| undefined` | `Result<User, NotFoundError>` |
| `return undefined` | `if (!found) return undefined` | `return { ok: false, error: ... }` |
| Swallowed catch | `catch { return undefined }` | `return { ok: false, error }` or re-throw |
| Silent failure | `result?.value` without error handling | Check `result.ok` explicitly |

### Common Fuzzy Patterns

```typescript
// WRONG - fuzzy return
function findUser(id: string): User | undefined {
  return users.get(id);  // undefined if not found - FUZZY
}

// WRONG - swallowed error
async function fetch(): Promise<Data | undefined> {
  try {
    return await api.get();
  } catch {
    return undefined;  // Error swallowed - FUZZY
  }
}

// RIGHT - explicit Result
function findUser(id: UserId): Result<User, UserError> {
  const user = users.get(id);
  if (!user) return { ok: false, error: { kind: "not_found", id } };
  return { ok: true, value: user };
}

// RIGHT - explicit error handling
async function fetch(): Promise<Result<Data, ApiError>> {
  try {
    const data = await api.get();
    return { ok: true, value: data };
  } catch (error) {
    return { ok: false, error: { kind: "fetch_failed", cause: error } };
  }
}
```

**Rule:** If a function can fail, the failure MUST be explicit in the return type. Callers MUST handle both cases.

### Frontend Exception

In React/Zustand frontend code, `T | undefined` and `return undefined` are ALLOWED for:
- Zustand store state (loading states, optional selections)
- React useState hooks
- Helper functions that return undefined for "not found" (caller filters)
- Optional component props

These patterns are NOT allowed in:
- Service layer functions (must use Result<T, E>)
- API response handlers (must use Result<T, E>)
- Validation functions (must use Result<T, ValidationError>)

---

## 3. CRITICAL: Zero Comments Policy

Comments indicate failure to write self-documenting code. They lie, rot, and mislead.

### Forbidden Comment Types

| Type | Example | Resolution |
|------|---------|------------|
| Inline explanations | `// increment counter` | Delete - code is obvious |
| TODO/FIXME/HACK | `// TODO: fix this later` | Fix now or delete |
| Changelog comments | `// Added by John, 2024` | Delete - git has history |
| Commented-out code | `// return oldValue;` | Delete - git has history |
| Section dividers | `// ========= UTILS =========` | Use proper file structure |
| Obvious JSDoc | `/** Gets the user */` on `getUser()` | Delete - name is clear |
| Type descriptions | `/** The user's ID */` on `userId: UserId` | Delete - type is documentation |
| Parameter docs | `@param id - the id` | Delete - parameter name is clear |
| Return docs | `@returns the user` | Delete - return type is clear |
| Magic number explanations | `const x = 86400; // seconds in day` | Use named constant instead |

### Allowed Exceptions (RARE)

| Exception | When Allowed | Example |
|-----------|--------------|---------|
| Regex explanation | Complex regex that cannot be simplified | `/(?=.*[A-Z])/ matches uppercase` |
| External reference | Link to spec/RFC that code implements | `// Per RFC 7231 Section 6.1` |
| Non-obvious business rule | Legal/compliance requirement | `// GDPR requires 30-day retention` |
| Performance justification | Counter-intuitive optimization | `// O(1) lookup via hash, not O(n) scan` |

### Self-Documenting Replacements

| Bad (with comment) | Good (self-documenting) |
|--------------------|-------------------------|
| `const t = 86400; // seconds in day` | `const SECONDS_PER_DAY = 86400;` |
| `if (s === 1) // active status` | `if (status === UserStatus.Active)` |
| `// validate email format` followed by regex | `function isValidEmailFormat(email: string)` |
| `// retry 3 times` | `const MAX_RETRY_ATTEMPTS = 3;` |
| `arr.filter(x => x > 0) // remove negatives` | `const positiveValues = arr.filter(isPositive);` |

**Rule:** If you need a comment, you need better names. Fix the code, not the symptom.

---

## 4. CRITICAL: Zero Duplication

Duplication is the root of maintenance nightmares. Every duplicated line will diverge.

### Detection Methods

| Method | What It Finds |
|--------|---------------|
| `jscpd` | Copy-pasted code blocks |
| ESLint `no-duplicate-case` | Switch case duplicates |
| Manual review | Similar logic with slight variations |
| Type inspection | Identical or near-identical type definitions |

### Common Duplication Patterns

| Pattern | Example | Resolution |
|---------|---------|------------|
| Copy-pasted functions | Same logic in 2+ places | Extract to shared utility |
| Near-identical types | `UserResponse` vs `UserDTO` with same fields | Single canonical type |
| Repeated validation | Email regex in multiple files | Single `isValidEmail` function |
| Similar API handlers | CRUD handlers with same structure | Extract to factory or generic |
| Repeated error handling | Same try-catch in every function | Higher-order function or Result pattern |
| Config scattered | Same values in multiple places | Single config source |
| Duplicate fetch logic | Same HTTP calls with minor variations | Generic fetch utility |

### Type Duplication

```typescript
// WRONG - duplicate type definitions
// file1.ts
interface User { id: string; name: string; email: string; }

// file2.ts
type UserData = { id: string; name: string; email: string; }

// file3.ts
interface UserRecord { id: string; name: string; email: string; }

// RIGHT - single source of truth
// types/user.ts
export interface User {
  readonly id: UserId;
  readonly name: string;
  readonly email: Email;
}

// Use everywhere
import type { User } from "@/types/user";
```

### Logic Duplication

```typescript
// WRONG - duplicated validation
function createUser(email: string) {
  if (!email.includes("@")) throw new Error("Invalid");
  // ...
}

function updateEmail(email: string) {
  if (!email.includes("@")) throw new Error("Invalid");
  // ...
}

// RIGHT - extracted validator
function validateEmail(email: string): Result<Email, ValidationError> {
  if (!email.includes("@")) {
    return { ok: false, error: { kind: "invalid_email", value: email } };
  }
  return { ok: true, value: email as Email };
}

function createUser(email: string): Result<User, UserError> {
  const emailResult = validateEmail(email);
  if (!emailResult.ok) return { ok: false, error: emailResult.error };
  // ...
}
```

### Structural Duplication

```typescript
// WRONG - repeated CRUD patterns
const getUser = async (id: string) => { /* fetch logic */ };
const getOrder = async (id: string) => { /* same fetch logic */ };
const getProduct = async (id: string) => { /* same fetch logic */ };

// RIGHT - generic pattern
function createFetcher<T, Id extends string>(
  endpoint: string,
): (id: Id) => Promise<Result<T, ApiError>> {
  return async (id) => {
    const response = await fetch(`${endpoint}/${id}`);
    if (!response.ok) {
      return { ok: false, error: { kind: "fetch_failed", status: response.status } };
    }
    return { ok: true, value: await response.json() };
  };
}

const getUser = createFetcher<User, UserId>("/api/users");
const getOrder = createFetcher<Order, OrderId>("/api/orders");
```

**Rule:** If you write it twice, extract it once. DRY is non-negotiable.

---

## 5. Architecture: Zero Redundancy

| Check | Action |
|-------|--------|
| Duplicate functionality | Merge into single authoritative module |
| Similar interfaces/types | Consolidate or extract shared types |
| Copy-pasted logic | Extract to shared utility or higher-order function |
| Multiple files doing same thing | Delete redundant, keep one source of truth |
| Unused modules/files | Delete immediately |
| Dead code paths | Delete - git has history |

**Rule:** If two things do the same job, one must die.

---

## 6. File & Folder Consistency

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Files | `kebab-case.ts` | `user-service.ts` |
| Directories | `kebab-case` | `auth/`, `mcp-servers/` |
| Type files | Named after primary type | `user.ts` for `User` |
| Index files | Re-exports only | `index.ts` exports public API |
| Test files | Same name with `.test.ts` | `user-service.test.ts` |

### Grouping Rules

| Pattern | Structure |
|---------|-----------|
| Feature module | `feature/index.ts`, `feature/types.ts`, `feature/service.ts`, `feature/repository.ts` |
| Shared types | `src/types/` |
| Utilities | `src/utils/` with specific names |
| Constants | `src/constants/` |

### No Orphaned Files

| Rule | Enforcement |
|------|-------------|
| Every `.ts` file lives in a domain folder | No loose files at `src/` root except `index.ts` |
| Small/limited functionality | Merge into related domain file |
| "Doesn't fit anywhere" | It fits somewhere. Find the domain. |

### Forbidden

| Violation | Resolution |
|-----------|------------|
| `utils.ts` (catch-all) | Name by actual purpose: `string-utils.ts`, `date-utils.ts` |
| `helpers.ts` / `common.ts` | Categorize properly or delete |
| `misc/` / `other/` directories | Categorize properly or delete |
| Multiple barrel files | One `index.ts` per module |
| Re-exports across domains | Import from source directly |

---

## 7. Module Boundaries

| Layer | Responsibility | Dependencies |
|-------|----------------|--------------|
| `handlers/` | HTTP/event handlers, request/response | Services |
| `services/` | Business logic | Repositories, external clients |
| `repositories/` | Data access | Database client only |
| `types/` | Data structures | None (leaf nodes) |
| `utils/` | Pure utilities | None (leaf nodes) |

**Rule:** Dependencies flow downward. Never upward. Never sideways between peers.

---

## 8. Tech Debt Policy

| Item | Policy |
|------|--------|
| TODO comments | Forbidden - fix now or delete |
| FIXME comments | Forbidden - fix now or delete |
| `@ts-ignore` | Forbidden - fix the type error |
| `any` type | Forbidden - use proper types |
| "We'll refactor later" | No. Refactor now. |
| Commented-out code | Delete immediately |
| Console.log statements | Delete before commit |
| Unused dependencies | Remove from package.json |

**Rule:** Tech debt is not accepted. Ever. Ship clean or don't ship.

---

## Review Checklist

**MANDATORY:** Each item must be explicitly checked. No silent passes.

### Dead Code (Highest Priority)
- [ ] Zero unused exports (verified with `ts-prune`)
- [ ] Zero unused imports
- [ ] Zero unused functions/types (IDE "Find References")
- [ ] Zero unreachable code paths
- [ ] Zero console.log statements
- [ ] Zero commented-out code

### No Fuzzy Data (CRITICAL)
- [ ] Zero `T | undefined` return types (use `Result<T, E>`)
- [ ] Zero `return undefined` statements (return Result or throw)
- [ ] Zero swallowed errors in catch blocks (return Result or re-throw)
- [ ] All failures explicit in return type
- [ ] All function callers handle both success and failure cases

### Comments (Zero Tolerance)
- [ ] Zero inline comments (`//`)
- [ ] Zero TODO/FIXME/HACK comments
- [ ] Zero obvious JSDoc (type is documentation)
- [ ] Zero changelog/author comments
- [ ] Zero section dividers

### Duplication (Zero Tolerance)
- [ ] Zero duplicate type definitions
- [ ] Zero copy-pasted logic blocks
- [ ] Zero near-identical functions
- [ ] Zero repeated validation logic
- [ ] Zero scattered config values

### Structure
- [ ] All file/folder names follow conventions
- [ ] No orphaned files - every file in proper folder
- [ ] No catch-all files (utils.ts, helpers.ts)
- [ ] Module boundaries respected
- [ ] Dependencies flow downward only

### TypeScript Quality
- [ ] Zero `any` types
- [ ] Zero `as` assertions (use type guards)
- [ ] Zero `!` non-null assertions
- [ ] Zero `@ts-ignore` / `@ts-expect-error`
- [ ] All identifiers use branded types
- [ ] All optional states use discriminated unions
- [ ] All functions return Result or throw (no fuzzy undefined)
- [ ] Complies with `typescript.md` standards

---

## Review Output Format

```
## Review: [PR/File Name]

### Dead Code
- Unused exports: None / Found: [list]
- Unused imports: None / Found: [list]
- Unused functions: None / Found: [list]
- Commented-out code: None / Found: [list with line numbers]
- Console.log: None / Found: [list]

### Fuzzy Data (CRITICAL)
- `T | undefined` return types: None / Found: [list with file:line]
- `return undefined` statements: None / Found: [list with file:line]
- Swallowed errors: None / Found: [list catch blocks]
- Unhandled Result cases: None / Found: [list callers not checking .ok]

### Comments
- Inline comments: None / Found: [count, list locations]
- TODO/FIXME: None / Found: [list with line numbers]
- Obvious JSDoc: None / Found: [list]

### Duplication
- Duplicate types: None / Found: [list files]
- Duplicate logic: None / Found: [describe, list files]
- Near-identical functions: None / Found: [list]

### Structure
- Orphaned files: None / Found: [list]
- Catch-all files: None / Found: [list]
- Boundary violations: None / Found: [describe]

### TypeScript Quality
- `any` usage: None / Found: [count, locations]
- Type assertions: None / Found: [list]
- Non-null assertions: None / Found: [list]
- `typescript.md` compliance: Full / Violations: [list]

**Verdict:** APPROVE / REJECT
**Required changes:** [if rejected, list specific actions with file:line]
```

---

## Automated Enforcement

Add to CI pipeline:

```json
{
  "scripts": {
    "lint:unused": "ts-prune && depcheck",
    "lint:types": "tsc --noEmit --noUnusedLocals --noUnusedParameters",
    "lint:code": "eslint . --max-warnings 0",
    "lint:duplicates": "jscpd src --threshold 0",
    "lint:all": "npm run lint:unused && npm run lint:types && npm run lint:code && npm run lint:duplicates"
  }
}
```

**Rule:** If automation can catch it, humans shouldn't need to. Automate everything.
