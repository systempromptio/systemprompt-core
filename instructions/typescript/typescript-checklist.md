# TypeScript Code Review Checklist

**Purpose:** Systematic validation of TypeScript modules against project standards.

**References:**
- `/instructions/typescript.md` - Code-level standards
- `/instructions/typescript-review.md` - Review standards

---

## Agent Instructions

When reviewing a target module or directory:

1. Read all source files in the target
2. Validate each rule in the checklist below
3. Record findings with evidence (file:line references)
4. **MANDATORY: Create `status.md` in the module root**
5. Write the complete review results table to `status.md`
6. Provide verdict and required actions

**CRITICAL REQUIREMENT:**

You MUST create a file called `status.md` in the module root directory. This file MUST contain:

- The complete results table with all 52 checks
- Pass/Fail status for each rule
- Evidence (file:line references) for failures
- Summary counts
- Verdict
- Timestamp of review

This is NOT optional. Every review MUST produce a `status.md` file.

---

## Checklist

### Section 1: Dead Code Elimination (CRITICAL)

| ID | Rule | Check |
|----|------|-------|
| T1.1 | No unused exports | Run `ts-prune` or check IDE references |
| T1.2 | No unused imports | ESLint `no-unused-imports` |
| T1.3 | No unused functions | IDE "Find References" shows 0 usages |
| T1.4 | No unused types/interfaces | IDE "Find References" shows 0 usages |
| T1.5 | No unused parameters | Prefix with `_` only if interface required |
| T1.6 | No unreachable code | Code after `return`, `throw`, `break`, `continue` |
| T1.7 | No commented-out code | Search for `// const`, `// function`, `// return` patterns |
| T1.8 | No console.log statements | Search for `console.log` |
| T1.9 | No backup/old functions | Search for `Old`, `Backup`, `V1`, `V2` suffixes |
| T1.10 | No empty catch blocks | Search for `catch (e) {}` or `catch {}` with no body |

### Section 2: No Fuzzy Data Policy (CRITICAL)

| ID | Rule | Check |
|----|------|-------|
| T2.1 | No `T \| undefined` return types | Backend: use Result. Frontend: allowed for state (CONDITIONAL) |
| T2.2 | No `return undefined` statements | Backend: use Result. Frontend: allowed for helpers (CONDITIONAL) |
| T2.3 | No swallowed errors in catch | Must return Result or re-throw |
| T2.4 | All failures explicit in return type | Backend: required. Frontend: optional for state (CONDITIONAL) |
| T2.5 | Callers handle both success/failure | Check Result.ok checks at call sites |
| T2.6 | No silent `?.` without error handling | Optional chaining must have fallback logic |

### Section 3: Zero Comments Policy (CRITICAL)

| ID | Rule | Check |
|----|------|-------|
| T3.1 | No inline comments (`//`) | Search for `//` excluding eslint directives |
| T3.2 | No TODO comments | Search for `TODO` |
| T3.3 | No FIXME comments | Search for `FIXME` |
| T3.4 | No HACK comments | Search for `HACK` |
| T3.5 | No obvious JSDoc | `/** Gets the user */` on `getUser()` |
| T3.6 | No changelog/author comments | `// Added by`, `// Modified` |
| T3.7 | No section dividers | `// ========` patterns |
| T3.8 | No type description comments | `/** The user's ID */` on typed fields |

### Section 4: Zero Duplication (CRITICAL)

| ID | Rule | Check |
|----|------|-------|
| T4.1 | No duplicate type definitions | Same interface in multiple files |
| T4.2 | No copy-pasted logic blocks | Run `jscpd` or manual inspection |
| T4.3 | No near-identical functions | Functions with 80%+ similarity |
| T4.4 | No repeated validation logic | Same regex/validation in multiple places |
| T4.5 | No scattered config values | Magic strings/numbers repeated |
| T4.6 | No duplicate fetch/API logic | Similar HTTP calls with minor variations |

### Section 5: TypeScript Quality (CRITICAL)

| ID | Rule | Check |
|----|------|-------|
| T5.1 | No `any` type | Search for `: any` or `as any` |
| T5.2 | No `as` type assertions | Forbidden except: branded types, discriminated dispatch, validated JSON (CONDITIONAL) |
| T5.3 | No `!` non-null assertions | Search for `!.` or `!;` patterns |
| T5.4 | No `@ts-ignore` | Search for `@ts-ignore` |
| T5.5 | No `@ts-expect-error` | Search for `@ts-expect-error` |
| T5.6 | Branded types for identifiers | `UserId`, `OrderId` not plain `string` |
| T5.7 | Discriminated unions for optionals | `{ status: 'loading' } \| { status: 'done', data: T }` |
| T5.8 | `readonly` on immutable fields | Required for shared types, optional for React state (OPTIONAL) |

### Section 6: File & Folder Consistency

| ID | Rule | Check |
|----|------|-------|
| T6.1 | Files are `kebab-case.ts` | Check all `.ts`/`.tsx` file names |
| T6.2 | Directories are `kebab-case` | Check all folder names |
| T6.3 | No `utils.ts` catch-all | Must be specific: `string-utils.ts` |
| T6.4 | No `helpers.ts` / `common.ts` | Forbidden file names |
| T6.5 | No `misc/` / `other/` directories | Forbidden folder names |
| T6.6 | No orphaned files at src root | Only `index.ts` at root |
| T6.7 | Test files use `.test.ts` suffix | Consistent test naming |

### Section 7: Module Boundaries

| ID | Rule | Check |
|----|------|-------|
| T7.1 | Dependencies flow downward only | No upward or circular imports |
| T7.2 | Types have no dependencies | Leaf nodes only |
| T7.3 | Utils have no dependencies | Pure functions only |
| T7.4 | Services use repositories | No direct DB/API calls in handlers |
| T7.5 | No cross-domain direct imports | Use shared types module |

### Section 8: Tech Debt Policy

| ID | Rule | Check |
|----|------|-------|
| T8.1 | No TODO comments | Forbidden - fix now or delete |
| T8.2 | No `@ts-ignore` | Forbidden - fix the type error |
| T8.3 | No `any` type | Forbidden - use proper types |
| T8.4 | No unused dependencies | Check `package.json` with `depcheck` |

---

## Review Output Format

**MANDATORY:** Create `status.md` in the module root with the following content:

```markdown
# Code Review Status

**Module:** [Module Name]
**Reviewed:** [YYYY-MM-DD HH:MM UTC]
**Reviewer:** Claude Code Agent

## Results

| ID | Rule | Status | Evidence |
|----|------|--------|----------|
| T1.1 | No unused exports | PASS/FAIL | [file:line or "None found"] |
| T1.2 | No unused imports | PASS/FAIL | [file:line or "None found"] |
| ... | ... | ... | ... |

### Summary

| Category | Pass | Fail | Total |
|----------|------|------|-------|
| Dead Code Elimination | X | Y | 10 |
| No Fuzzy Data | X | Y | 6 |
| Zero Comments | X | Y | 8 |
| Zero Duplication | X | Y | 6 |
| TypeScript Quality | X | Y | 8 |
| File & Folder | X | Y | 7 |
| Module Boundaries | X | Y | 5 |
| Tech Debt | X | Y | 4 |
| **Total** | X | Y | 52 |

### Verdict

**Status:** APPROVED / REJECTED

## Required Actions

[If rejected, list specific fixes required with file:line references]

1. [Action 1]
2. [Action 2]
...
```

**REMINDER:** This entire content block MUST be written to `status.md` in the module root.

---

## Execution Commands

Run these commands as part of validation:

```bash
# Check for forbidden patterns
grep -rn "console\.log" --include="*.ts" --include="*.tsx" [target]
grep -rn ": any" --include="*.ts" --include="*.tsx" [target]
grep -rn "as any" --include="*.ts" --include="*.tsx" [target]
grep -rn "@ts-ignore" --include="*.ts" --include="*.tsx" [target]
grep -rn "@ts-expect-error" --include="*.ts" --include="*.tsx" [target]
grep -rn "TODO\|FIXME\|HACK" --include="*.ts" --include="*.tsx" [target]
grep -rn "\.unwrap()" --include="*.ts" --include="*.tsx" [target]

# Check for fuzzy data patterns
grep -rn "| undefined" --include="*.ts" --include="*.tsx" [target]
grep -rn "return undefined" --include="*.ts" --include="*.tsx" [target]

# Check for non-null assertions
grep -rn "\!\\." --include="*.ts" --include="*.tsx" [target]
grep -rn "\!;" --include="*.ts" --include="*.tsx" [target]

# Check file structure
find [target] -name "utils.ts" -o -name "helpers.ts" -o -name "common.ts"
find [target] -type d -name "misc" -o -name "other"

# TypeScript compilation
npx tsc --noEmit --noUnusedLocals --noUnusedParameters

# ESLint
npx eslint [target] --max-warnings 0

# Find unused exports
npx ts-prune [target]

# Find unused dependencies
npx depcheck

# Find duplicates
npx jscpd [target] --threshold 0

# Count lines per file (max 300)
find [target] -name "*.ts" -o -name "*.tsx" | xargs wc -l | awk '$1 > 300 {print}'
```

---

## Approval Criteria

A module is **APPROVED** only when:

1. Zero FAIL results in Dead Code Elimination (T1.x)
2. Zero FAIL results in No Fuzzy Data (T2.x)
3. Zero FAIL results in Zero Comments (T3.x)
4. Zero FAIL results in TypeScript Quality (T5.x)
5. All other sections have ≤2 FAIL results with documented exceptions

A module is **REJECTED** if:

1. Any Dead Code violation exists
2. Any Fuzzy Data violation exists
3. Any forbidden comment exists
4. Any `any` type or `@ts-ignore` exists
5. Any forbidden file/folder pattern exists
6. More than 2 violations in any other section

---

## Final Checklist

Before completing the review, verify:

- [ ] All 52 rules have been checked
- [ ] `status.md` has been created in the module root
- [ ] `status.md` contains the complete results table
- [ ] `status.md` contains the timestamp
- [ ] `status.md` contains the verdict
- [ ] Required actions are listed (if rejected)

**DO NOT consider the review complete until `status.md` exists.**
