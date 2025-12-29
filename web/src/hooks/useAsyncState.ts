import { useState, useCallback } from 'react'
import { logger } from '@/lib/logger'

export type AsyncState<T> =
  | { status: 'idle'; data: undefined; error: undefined; loading: false }
  | { status: 'loading'; data: undefined; error: undefined; loading: true }
  | { status: 'success'; data: T; error: undefined; loading: false }
  | { status: 'error'; data: undefined; error: Error; loading: false }

export interface UseAsyncStateOptions {
  onSuccess?: (data: unknown) => void
  onError?: (error: Error) => void
  moduleId?: string
}

export function useAsyncState<T>(
  fn: () => Promise<T>,
  options: UseAsyncStateOptions = {}
) {
  const { onSuccess, onError, moduleId = 'useAsyncState' } = options

  const [state, setState] = useState<AsyncState<T>>({
    status: 'idle',
    data: undefined,
    error: undefined,
    loading: false
  })

  const execute = useCallback(async (): Promise<T | undefined> => {
    setState({
      status: 'loading',
      data: undefined,
      error: undefined,
      loading: true
    })

    try {
      const result = await fn()

      setState({
        status: 'success',
        data: result,
        error: undefined,
        loading: false
      })

      onSuccess?.(result)
      logger.debug('Async operation succeeded', undefined, moduleId)

      return result
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err))

      setState({
        status: 'error',
        data: undefined,
        error,
        loading: false
      })

      onError?.(error)
      logger.error('Async operation failed', error, moduleId)

      throw error
    }
  }, [fn, onSuccess, onError, moduleId])

  const reset = useCallback(() => {
    setState({
      status: 'idle',
      data: undefined,
      error: undefined,
      loading: false
    })
    logger.debug('Async state reset', undefined, moduleId)
  }, [moduleId])

  return {
    ...state,
    execute,
    reset,
    isLoading: state.loading,
    isSuccess: state.status === 'success',
    isError: state.status === 'error',
  }
}
