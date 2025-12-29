# SystemPrompt TypeScript Standards

**SystemPrompt is a world-class TypeScript programming brand.** Every TypeScript file in this codebase must be instantly recognizable as on-brand, world-class idiomatic TypeScript. No exceptions. No shortcuts. No compromise.

Checkable, actionable patterns. Run `tsc --noEmit` and `eslint --fix` after changes.

---

## 1. Limits

| Metric | Limit |
|--------|-------|
| Source file length | 500 lines |
| Cognitive complexity | 15 |
| Function length | 50 lines |
| Parameters | 5 |

---

## 2. Forbidden Constructs

| Construct | Resolution |
|-----------|------------|
| `any` | Use proper types, generics, or `unknown` with type guards |
| `null` | Use `undefined` exclusively for optional fields |
| `undefined` as return value | Return `Result<T, E>` or throw - NO FUZZY DATA |
| `T \| undefined` return type | Return `Result<T, E>` - caller must handle explicitly |
| `undefined` checks scattered | Use discriminated unions or Result pattern |
| `as` type assertions | Use type guards or redesign types |
| `!` non-null assertion | Use proper narrowing or discriminated unions |
| `// @ts-ignore` / `// @ts-expect-error` | Fix the type error properly |
| Inline comments (`//`) | ZERO TOLERANCE - delete all. Code documents itself through naming and structure |
| JSDoc comments (`/** */`) | ZERO TOLERANCE - types are the documentation |
| TODO/FIXME/HACK comments | Fix immediately or don't write |
| `Object` / `{}` / `object` types | Use specific interfaces or `Record<K, V>` |
| `Function` type | Use specific function signatures |
| Enums | Use discriminated unions or const objects |
| Classes for data | Use interfaces + factory functions |
| `try/catch` for control flow | Use Result pattern |

### Frontend Exceptions (React/Zustand)

In React/Zustand frontend code, the following patterns are ALLOWED:

| Pattern | When Allowed | NOT Allowed |
|---------|--------------|-------------|
| `T \| undefined` | Zustand store state, React useState, optional props | Service return types, API handlers |
| `return undefined` | Helper functions used with `.filter()` | Service methods, validators |
| `?.` chaining | Safe property access with fallback | Without fallback handling |

### Allowed Type Assertions

Type assertions (`as`) are permitted in these specific cases:

| Pattern | When Allowed | Example |
|---------|--------------|---------|
| Branded type creation | Factory functions for branded types | `return id as UserId` |
| Discriminated dispatch | After switch/if on discriminator | `hints as TableHints` |
| Runtime-validated JSON | After Zod/schema validation passes | `schema.parse(json) as T` |

**Forbidden:** Silencing type errors, casting `any`, bypassing checks without validation.

**CRITICAL:** `response.json() as T` is NOT validated JSON. External API responses MUST be validated with a runtime schema (Zod, io-ts, etc.) before type assertion. The `as` assertion is only safe AFTER validation succeeds.

---

## 3. Mandatory Patterns

### NO FUZZY DATA - Core Principle

**Functions NEVER return `undefined` to indicate absence or failure.** Every function either:
1. Returns the actual value (success)
2. Returns a `Result<T, E>` discriminated union (expected failures)
3. Throws an error (unexpected/programming errors)

```typescript
// WRONG - fuzzy return, caller doesn't know why it failed
function getUser(id: string): User | undefined {
  const user = db.find(id);
  return user; // undefined if not found - FUZZY
}

// WRONG - caller can ignore the error case
function fetchData(): Data | undefined {
  try {
    return api.fetch();
  } catch {
    return undefined; // Swallowed error - FUZZY
  }
}

// RIGHT - explicit success/failure, caller MUST handle both
function getUser(id: UserId): Result<User, UserError> {
  const user = db.find(id);
  if (!user) return { ok: false, error: { kind: "not_found", id } };
  return { ok: true, value: user };
}

// RIGHT - unexpected errors throw, expected failures return Result
function fetchData(): Result<Data, ApiError> {
  const response = await fetch(url);
  if (!response.ok) {
    return { ok: false, error: { kind: "http_error", status: response.status } };
  }
  return { ok: true, value: await response.json() };
}
```

**Rule:** If a function can fail, the failure MUST be explicit in the return type. No silent `undefined`. No swallowed errors. No fuzzy data.

### The Result Pattern

Never throw exceptions for expected failures. Use discriminated unions:

```typescript
// WRONG
function getUser(id: string): User {
  const user = db.find(id);
  if (!user) throw new Error("Not found");
  return user;
}

// RIGHT
type Result<T, E = Error> =
  | { ok: true; value: T }
  | { ok: false; error: E };

function getUser(id: UserId): Result<User, UserError> {
  const user = db.find(id);
  if (!user) return { ok: false, error: { kind: "not_found", id } };
  return { ok: true, value: user };
}
```

### Branded Types for Identifiers

All identifier fields use branded types to prevent mixing:

```typescript
// WRONG
interface Task {
  id: string;
  userId: string;
}

// RIGHT
type Brand<T, B> = T & { readonly __brand: B };

type TaskId = Brand<string, "TaskId">;
type UserId = Brand<string, "UserId">;

interface Task {
  id: TaskId;
  userId: UserId;
}

const createTaskId = (id: string): TaskId => id as TaskId;
const createUserId = (id: string): UserId => id as UserId;
```

### Discriminated Unions Over Optional Fields

Never use optional fields when states are mutually exclusive:

```typescript
// WRONG - impossible states representable
interface ApiResponse<T> {
  data?: T;
  error?: string;
  loading?: boolean;
}

// RIGHT - only valid states representable
type ApiResponse<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; data: T }
  | { status: "error"; error: AppError };
```

### Exhaustive Pattern Matching

Always handle all cases with exhaustiveness checking:

```typescript
function assertNever(value: never): never {
  throw new Error(`Unhandled case: ${JSON.stringify(value)}`);
}

function handleResponse<T>(response: ApiResponse<T>): string {
  switch (response.status) {
    case "idle":
      return "Ready";
    case "loading":
      return "Loading...";
    case "success":
      return `Got: ${response.data}`;
    case "error":
      return `Error: ${response.error.message}`;
    default:
      return assertNever(response);
  }
}
```

### When to Throw vs Return Result

| Scenario | Pattern | Example |
|----------|---------|---------|
| Expected failure (user input, external API) | `Result<T, E>` | User not found, validation failed |
| Programming error (bug, invalid state) | `throw Error` | Null pointer, assertion failure |
| Unrecoverable error | `throw Error` | Out of memory, configuration missing |

```typescript
// Expected failure - use Result
function parseEmail(input: string): Result<Email, ValidationError> {
  if (!input.includes("@")) {
    return { ok: false, error: { kind: "invalid_format" } };
  }
  return { ok: true, value: input as Email };
}

// Programming error - throw
function getRequiredConfig(key: string): string {
  const value = process.env[key];
  if (!value) {
    throw new Error(`Missing required config: ${key}`);
  }
  return value;
}
```

### Strict Function Signatures

Never use optional parameters when a function has multiple behaviors:

```typescript
// WRONG - unclear behavior
function fetchData(url: string, cache?: boolean, timeout?: number): Promise<Data>;

// RIGHT - explicit overloads or separate functions
function fetchData(url: string, options: FetchOptions): Promise<Data>;

interface FetchOptions {
  cache: CacheStrategy;
  timeout: Milliseconds;
}
```

### Readonly by Default

All data structures are immutable unless mutation is explicitly required:

```typescript
// WRONG
interface Config {
  apiUrl: string;
  features: string[];
}

// RIGHT
interface Config {
  readonly apiUrl: string;
  readonly features: readonly string[];
}

type DeepReadonly<T> = {
  readonly [P in keyof T]: T[P] extends object ? DeepReadonly<T[P]> : T[P];
};
```

### Const Assertions for Literals

Use `as const` for literal types:

```typescript
// WRONG - type is string[]
const ROLES = ["admin", "user", "guest"];

// RIGHT - type is readonly ["admin", "user", "guest"]
const ROLES = ["admin", "user", "guest"] as const;
type Role = (typeof ROLES)[number];
```

### Builder Pattern for Complex Types

Required for types with 3+ fields OR any mix of required and optional:

```typescript
interface EmailMessage {
  readonly to: EmailAddress;
  readonly subject: string;
  readonly body: string;
  readonly cc: readonly EmailAddress[];
  readonly attachments: readonly Attachment[];
}

interface EmailBuilder {
  to(address: EmailAddress): EmailBuilder;
  subject(text: string): EmailBuilder;
  body(content: string): EmailBuilder;
  cc(addresses: readonly EmailAddress[]): EmailBuilder;
  attachments(files: readonly Attachment[]): EmailBuilder;
  build(): Result<EmailMessage, ValidationError>;
}

function createEmailBuilder(): EmailBuilder {
  let state: Partial<EmailMessage> = {
    cc: [],
    attachments: [],
  };

  const builder: EmailBuilder = {
    to: (address) => { state = { ...state, to: address }; return builder; },
    subject: (text) => { state = { ...state, subject: text }; return builder; },
    body: (content) => { state = { ...state, body: content }; return builder; },
    cc: (addresses) => { state = { ...state, cc: addresses }; return builder; },
    attachments: (files) => { state = { ...state, attachments: files }; return builder; },
    build: () => {
      if (!state.to || !state.subject || !state.body) {
        return { ok: false, error: { kind: "missing_required_fields" } };
      }
      return {
        ok: true,
        value: state as EmailMessage,
      };
    },
  };

  return builder;
}
```

### Repository Pattern

Services NEVER execute queries directly. All database access in repositories:

```typescript
// Repository - handles persistence, returns Result for all operations
interface UserRepository {
  findByEmail(email: Email): Promise<Result<User, RepositoryError>>;
  findById(id: UserId): Promise<Result<User, RepositoryError>>;
  create(user: CreateUserInput): Promise<Result<User, RepositoryError>>;
  update(id: UserId, data: UpdateUserInput): Promise<Result<User, RepositoryError>>;
  delete(id: UserId): Promise<Result<void, RepositoryError>>;
}

type RepositoryError =
  | { kind: "not_found"; id: string }
  | { kind: "duplicate"; field: string }
  | { kind: "connection_failed"; cause: unknown };

// Service - business logic, calls repository
class UserService {
  constructor(private readonly userRepository: UserRepository) {}

  async getUser(id: UserId): Promise<Result<User, ServiceError>> {
    const result = await this.userRepository.findById(id);
    if (!result.ok) {
      return { ok: false, error: { kind: "user_not_found", id } };
    }
    return { ok: true, value: result.value };
  }
}
```

### Type-Safe Error Handling

Define specific error types per domain:

```typescript
type UserError =
  | { kind: "not_found"; id: UserId }
  | { kind: "invalid_email"; email: string }
  | { kind: "duplicate_email"; email: Email }
  | { kind: "unauthorized"; action: string };

type ApiError =
  | { kind: "network"; cause: unknown }
  | { kind: "timeout"; ms: number }
  | { kind: "parse"; body: string };

function isUserError(error: unknown): error is UserError {
  return (
    typeof error === "object" &&
    error !== undefined &&
    "kind" in error &&
    typeof (error as UserError).kind === "string"
  );
}
```

### Dependency Injection via Functions

Prefer function composition over classes:

```typescript
// WRONG - class with injected dependencies
class UserService {
  constructor(
    private db: Database,
    private logger: Logger,
    private cache: Cache,
  ) {}
}

// RIGHT - factory function returning interface
type UserService = {
  getUser: (id: UserId) => Promise<Result<User, UserError>>;
  createUser: (input: CreateUserInput) => Promise<Result<User, UserError>>;
};

function createUserService(deps: {
  readonly db: Database;
  readonly logger: Logger;
  readonly cache: Cache;
}): UserService {
  return {
    getUser: async (id) => { /* implementation */ },
    createUser: async (input) => { /* implementation */ },
  };
}
```

---

## 4. Naming

### Functions

| Prefix | Returns | Failure Mode |
|--------|---------|--------------|
| `get` | `Result<T, NotFoundError>` | Result with not_found error |
| `find` | `Result<T, NotFoundError>` | Result with not_found error |
| `list` | `Result<readonly T[], E>` | Result with error (empty array is success) |
| `create` | `Result<T, E>` | Result with validation/conflict error |
| `update` | `Result<T, E>` | Result with not_found/validation error |
| `delete` | `Result<void, E>` | Result with not_found error |
| `is` / `has` | `boolean` | Never fails (type guards) |
| `to` | `T` | Throws on invalid input (programming error) |
| `from` | `Result<T, ParseError>` | Result with parse error |
| `parse` | `Result<T, ParseError>` | Result with parse error |
| `validate` | `Result<T, ValidationError>` | Result with validation error |

**Note:** NO function returns `T | undefined`. Every function either succeeds with `T`, returns `Result<T, E>`, or throws.

### Variables and Types

| Type | Naming Convention |
|------|-------------------|
| Interface/Type | PascalCase, noun: `User`, `ApiResponse` |
| Type parameter | Single uppercase or descriptive: `T`, `TInput`, `TOutput` |
| Branded type | PascalCase with domain: `UserId`, `EmailAddress` |
| Constant | SCREAMING_SNAKE_CASE: `MAX_RETRIES`, `API_BASE_URL` |
| Function | camelCase, verb-noun: `createUser`, `validateEmail` |
| Boolean | `is`, `has`, `should`, `can` prefix: `isActive`, `hasPermission` |
| Repository | `{noun}Repository`: `userRepository` |
| Service | `{noun}Service`: `authService` |

### Forbidden Abbreviations

`ctx`, `req`, `res`, `msg`, `err`, `cfg`, `cb`, `fn`, `obj`, `val` → Use full words.

Allowed: `id`, `url`, `api`, `http`, `json`, `sql`, `dto`, `db`

---

## 5. Advanced Type Patterns

### Mapped Types for Consistency

```typescript
type Handlers<Events extends Record<string, unknown>> = {
  [K in keyof Events]: (event: Events[K]) => void;
};

interface AppEvents {
  userCreated: { userId: UserId };
  userDeleted: { userId: UserId };
  orderPlaced: { orderId: OrderId; total: Money };
}

const handlers: Handlers<AppEvents> = {
  userCreated: (event) => { /* event.userId is typed */ },
  userDeleted: (event) => { /* event.userId is typed */ },
  orderPlaced: (event) => { /* event.orderId, event.total typed */ },
};
```

### Template Literal Types

```typescript
type HttpMethod = "GET" | "POST" | "PUT" | "DELETE";
type ApiPath = `/api/${string}`;
type Endpoint = `${HttpMethod} ${ApiPath}`;

const endpoint: Endpoint = "GET /api/users";
```

### Conditional Types for API Responses

```typescript
type ApiEndpoint<T extends string> =
  T extends "users" ? User[] :
  T extends "orders" ? Order[] :
  T extends `users/${string}` ? User :
  never;

function fetchApi<T extends string>(endpoint: T): Promise<ApiEndpoint<T>>;
```

### Infer for Type Extraction

```typescript
type UnwrapResult<T> = T extends Result<infer U, unknown> ? U : never;
type UnwrapOption<T> = T extends Option<infer U> ? U : never;
type UnwrapPromise<T> = T extends Promise<infer U> ? U : never;

type AwaitedResult<T> = UnwrapResult<UnwrapPromise<T>>;
```

---

## 6. Anti-Patterns

| Pattern | Resolution |
|---------|------------|
| `T \| undefined` return type | Return `Result<T, E>` - NO FUZZY DATA |
| `return undefined` on failure | Return `{ ok: false, error: ... }` |
| Swallowing errors in catch | Return Result with error, or re-throw |
| Raw string identifiers | Use branded types |
| Magic numbers/strings | Use const objects or enums-as-const |
| Direct database in services | Move to repository |
| Optional fields for states | Use discriminated unions |
| `try/catch` for expected errors | Use Result pattern |
| Mutable state | Use immutable updates |
| `any` to "fix" type errors | Fix the actual type issue |
| Type assertions (`as`) | Use type guards |
| Nested ternaries | Use early returns or switch |
| Callback hell | Use async/await with Result |
| `undefined` checks everywhere | Use Result pattern |
| Empty interfaces `{}` | Use `Record<string, never>` or remove |
| Unused code / dead code | Delete immediately |
| Commented-out code | Delete - git has history |

---

## 7. File Structure

```
src/
├── types/           # Shared type definitions, branded types
├── errors/          # Error type definitions per domain
├── utils/           # Pure utility functions (Result, Option, etc.)
├── repositories/    # Data access layer
├── services/        # Business logic layer
├── handlers/        # HTTP/event handlers
└── index.ts         # Public API exports
```

### Module Exports

Always use named exports, never default:

```typescript
// WRONG
export default function createUser() {}

// RIGHT
export function createUser(): Result<User, UserError> {}
```

---

## 8. Async Patterns

### Async Result Pattern

```typescript
type AsyncResult<T, E = Error> = Promise<Result<T, E>>;

const UserSchema = z.object({
  id: z.string(),
  name: z.string(),
  email: z.string(),
});

async function fetchUser(id: UserId): AsyncResult<User, ApiError> {
  const response = await fetch(`/api/users/${id}`);

  if (!response.ok) {
    return { ok: false, error: { kind: "network", status: response.status } };
  }

  const json = await response.json();
  const parsed = UserSchema.safeParse(json);

  if (!parsed.success) {
    return { ok: false, error: { kind: "parse", cause: parsed.error } };
  }

  return { ok: true, value: parsed.data };
}
```

### Sequential Operations with Results

```typescript
async function processOrder(
  orderId: OrderId,
): AsyncResult<ProcessedOrder, OrderError> {
  const orderResult = await getOrder(orderId);
  if (!orderResult.ok) return orderResult;

  const validationResult = await validateOrder(orderResult.value);
  if (!validationResult.ok) return validationResult;

  const paymentResult = await processPayment(validationResult.value);
  if (!paymentResult.ok) return paymentResult;

  return { ok: true, value: paymentResult.value };
}
```

### Parallel Operations

```typescript
async function loadDashboard(
  userId: UserId,
): AsyncResult<Dashboard, DashboardError> {
  const [userResult, ordersResult, statsResult] = await Promise.all([
    getUser(userId),
    getOrders(userId),
    getStats(userId),
  ]);

  if (!userResult.ok) return { ok: false, error: { kind: "user", error: userResult.error } };
  if (!ordersResult.ok) return { ok: false, error: { kind: "orders", error: ordersResult.error } };
  if (!statsResult.ok) return { ok: false, error: { kind: "stats", error: statsResult.error } };

  return {
    ok: true,
    value: {
      user: userResult.value,
      orders: ordersResult.value,
      stats: statsResult.value,
    },
  };
}
```
