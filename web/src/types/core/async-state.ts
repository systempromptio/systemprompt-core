type AsyncState<T, E = Error> =
  | { readonly status: 'idle' }
  | { readonly status: 'loading' }
  | { readonly status: 'success'; readonly data: T }
  | { readonly status: 'error'; readonly error: E }

function idle<T, E = Error>(): AsyncState<T, E> {
  return { status: 'idle' }
}

function loading<T, E = Error>(): AsyncState<T, E> {
  return { status: 'loading' }
}

function success<T, E = Error>(data: T): AsyncState<T, E> {
  return { status: 'success', data }
}

function error<T, E = Error>(errorValue: E): AsyncState<T, E> {
  return { status: 'error', error: errorValue }
}

function isIdle<T, E>(state: AsyncState<T, E>): state is { readonly status: 'idle' } {
  return state.status === 'idle'
}

function isLoading<T, E>(state: AsyncState<T, E>): state is { readonly status: 'loading' } {
  return state.status === 'loading'
}

function isSuccess<T, E>(state: AsyncState<T, E>): state is { readonly status: 'success'; readonly data: T } {
  return state.status === 'success'
}

function isError<T, E>(state: AsyncState<T, E>): state is { readonly status: 'error'; readonly error: E } {
  return state.status === 'error'
}

function getData<T, E>(state: AsyncState<T, E>): T | undefined {
  return state.status === 'success' ? state.data : undefined
}

function getError<T, E>(state: AsyncState<T, E>): E | undefined {
  return state.status === 'error' ? state.error : undefined
}

function mapAsyncState<T, U, E>(state: AsyncState<T, E>, fn: (data: T) => U): AsyncState<U, E> {
  if (state.status === 'success') {
    return success(fn(state.data))
  }
  return state as AsyncState<U, E>
}

export type { AsyncState }
export {
  idle,
  loading,
  success,
  error,
  isIdle,
  isLoading,
  isSuccess,
  isError,
  getData,
  getError,
  mapAsyncState,
}
