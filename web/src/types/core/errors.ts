type ValidationError = {
  readonly kind: 'validation'
  readonly field: string
  readonly path: readonly string[]
  readonly message: string
}

type NetworkError = {
  readonly kind: 'network'
  readonly status: number
  readonly message: string
}

type TimeoutError = {
  readonly kind: 'timeout'
  readonly durationMs: number
}

type ParseError = {
  readonly kind: 'parse'
  readonly message: string
  readonly input: string
}

type NotFoundError = {
  readonly kind: 'not_found'
  readonly resourceType: string
  readonly resourceId: string
}

type UnauthorizedError = {
  readonly kind: 'unauthorized'
  readonly message: string
}

type ForbiddenError = {
  readonly kind: 'forbidden'
  readonly action: string
  readonly resource: string
}

type ConflictError = {
  readonly kind: 'conflict'
  readonly message: string
}

type RateLimitError = {
  readonly kind: 'rate_limit'
  readonly retryAfterMs: number
}

type ServerError = {
  readonly kind: 'server_error'
  readonly status: number
  readonly message: string
}

type UnknownError = {
  readonly kind: 'unknown'
  readonly cause: unknown
}

type AppError =
  | ValidationError
  | NetworkError
  | TimeoutError
  | ParseError
  | NotFoundError
  | UnauthorizedError
  | ForbiddenError
  | ConflictError
  | RateLimitError
  | ServerError
  | UnknownError

type ApiError =
  | NetworkError
  | TimeoutError
  | ParseError
  | UnauthorizedError
  | ForbiddenError
  | RateLimitError
  | ServerError
  | UnknownError

type AuthError =
  | UnauthorizedError
  | ForbiddenError
  | NetworkError
  | ValidationError

type ArtifactError =
  | NotFoundError
  | ValidationError
  | NetworkError
  | ParseError

type TaskError =
  | NotFoundError
  | ValidationError
  | NetworkError
  | TimeoutError

type ContextError =
  | NotFoundError
  | ValidationError
  | NetworkError
  | ConflictError

function createValidationError(field: string, message: string, path: readonly string[] = []): ValidationError {
  return { kind: 'validation', field, path, message }
}

function createNetworkError(status: number, message: string): NetworkError {
  return { kind: 'network', status, message }
}

function createTimeoutError(durationMs: number): TimeoutError {
  return { kind: 'timeout', durationMs }
}

function createParseError(message: string, input: string): ParseError {
  return { kind: 'parse', message, input }
}

function createNotFoundError(resourceType: string, resourceId: string): NotFoundError {
  return { kind: 'not_found', resourceType, resourceId }
}

function createUnauthorizedError(message: string): UnauthorizedError {
  return { kind: 'unauthorized', message }
}

function createForbiddenError(action: string, resource: string): ForbiddenError {
  return { kind: 'forbidden', action, resource }
}

function createConflictError(message: string): ConflictError {
  return { kind: 'conflict', message }
}

function createRateLimitError(retryAfterMs: number): RateLimitError {
  return { kind: 'rate_limit', retryAfterMs }
}

function createServerError(status: number, message: string): ServerError {
  return { kind: 'server_error', status, message }
}

function createUnknownError(cause: unknown): UnknownError {
  return { kind: 'unknown', cause }
}

function assertNever(value: never): never {
  throw new Error(`Unhandled case: ${JSON.stringify(value)}`)
}

export type {
  ValidationError,
  NetworkError,
  TimeoutError,
  ParseError,
  NotFoundError,
  UnauthorizedError,
  ForbiddenError,
  ConflictError,
  RateLimitError,
  ServerError,
  UnknownError,
  AppError,
  ApiError,
  AuthError,
  ArtifactError,
  TaskError,
  ContextError,
}

export {
  createValidationError,
  createNetworkError,
  createTimeoutError,
  createParseError,
  createNotFoundError,
  createUnauthorizedError,
  createForbiddenError,
  createConflictError,
  createRateLimitError,
  createServerError,
  createUnknownError,
  assertNever,
}
