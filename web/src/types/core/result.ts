type Result<T, E = Error> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: E }

type AsyncResult<T, E = Error> = Promise<Result<T, E>>

function Ok<T>(value: T): Result<T, never> {
  return { ok: true, value }
}

function Err<E>(error: E): Result<never, E> {
  return { ok: false, error }
}

function isOk<T, E>(result: Result<T, E>): result is { readonly ok: true; readonly value: T } {
  return result.ok
}

function isErr<T, E>(result: Result<T, E>): result is { readonly ok: false; readonly error: E } {
  return !result.ok
}

function unwrap<T, E>(result: Result<T, E>): T {
  if (result.ok) {
    return result.value
  }
  throw new Error('Called unwrap on an Err value')
}

function unwrapOr<T, E>(result: Result<T, E>, defaultValue: T): T {
  return result.ok ? result.value : defaultValue
}

function mapResult<T, U, E>(result: Result<T, E>, fn: (value: T) => U): Result<U, E> {
  return result.ok ? Ok(fn(result.value)) : result
}

function mapError<T, E, F>(result: Result<T, E>, fn: (error: E) => F): Result<T, F> {
  return result.ok ? result : Err(fn(result.error))
}

async function mapAsyncResult<T, U, E>(
  result: AsyncResult<T, E>,
  fn: (value: T) => U | Promise<U>
): AsyncResult<U, E> {
  const resolved = await result
  if (!resolved.ok) return resolved
  return Ok(await fn(resolved.value))
}

function flatMap<T, U, E>(result: Result<T, E>, fn: (value: T) => Result<U, E>): Result<U, E> {
  return result.ok ? fn(result.value) : result
}

async function flatMapAsync<T, U, E>(
  result: Result<T, E>,
  fn: (value: T) => AsyncResult<U, E>
): AsyncResult<U, E> {
  return result.ok ? fn(result.value) : result
}

function fromTryCatch<T, E = Error>(fn: () => T, mapError?: (error: unknown) => E): Result<T, E> {
  try {
    return Ok(fn())
  } catch (error) {
    return Err(mapError ? mapError(error) : (error as E))
  }
}

async function fromAsyncTryCatch<T, E = Error>(
  fn: () => Promise<T>,
  mapError?: (error: unknown) => E
): AsyncResult<T, E> {
  try {
    return Ok(await fn())
  } catch (error) {
    return Err(mapError ? mapError(error) : (error as E))
  }
}

export type { Result, AsyncResult }
export {
  Ok,
  Err,
  isOk,
  isErr,
  unwrap,
  unwrapOr,
  mapResult,
  mapError,
  mapAsyncResult,
  flatMap,
  flatMapAsync,
  fromTryCatch,
  fromAsyncTryCatch,
}
