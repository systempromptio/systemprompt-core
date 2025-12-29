import { logger } from '@/lib/logger'

interface ErrorWithResponse {
  response?: {
    status?: number
    headers?: Record<string, string>
  }
}

function hasResponseStatus(error: unknown): error is ErrorWithResponse {
  if (typeof error !== 'object' || error === null) return false
  const errorRecord = error as Record<string, unknown>
  if (!errorRecord.response || typeof errorRecord.response !== 'object') return false
  return true
}

function getResponseStatus(error: unknown): number | undefined {
  if (!hasResponseStatus(error)) return undefined
  return error.response?.status
}

interface RetryOptions {
  maxRetries?: number
  baseDelay?: number
  maxDelay?: number
  shouldRetry?: (error: unknown) => boolean
}

const defaultOptions: Required<RetryOptions> = {
  maxRetries: 3,
  baseDelay: 1000,
  maxDelay: 10000,
  shouldRetry: (error: unknown) => {
    const status = getResponseStatus(error)
    if (status === 429) return true
    if (status !== undefined && status >= 500) return true
    return false
  },
}

export async function fetchWithRetry<T>(
  fn: () => Promise<T>,
  options: RetryOptions = {}
): Promise<T> {
  const opts = { ...defaultOptions, ...options }

  for (let attempt = 0; attempt < opts.maxRetries; attempt++) {
    try {
      return await fn()
    } catch (error: unknown) {
      const isLastAttempt = attempt === opts.maxRetries - 1

      if (isLastAttempt || !opts.shouldRetry(error)) {
        throw error
      }

      const retryAfterMs = getRetryAfter(error)
      let delay: number

      if (retryAfterMs !== undefined) {
        delay = retryAfterMs
      } else {
        delay = Math.min(opts.baseDelay * Math.pow(2, attempt), opts.maxDelay)
      }

      const jitter = Math.random() * 200
      const totalDelay = delay + jitter

      logger.debug(
        `Retry attempt ${attempt + 1}/${opts.maxRetries}`,
        { delayMs: Math.round(totalDelay), status: getResponseStatus(error) },
        'fetch-with-retry'
      )

      await new Promise((resolve) => setTimeout(resolve, totalDelay))
    }
  }

  throw new Error('Max retries exceeded')
}

export function isRateLimitError(error: unknown): boolean {
  return getResponseStatus(error) === 429
}

export function getRetryAfter(error: unknown): number | undefined {
  if (!hasResponseStatus(error)) return undefined
  const headers = error.response?.headers
  if (!headers) return undefined
  const retryAfter = headers['retry-after']
  if (retryAfter) {
    const seconds = parseInt(retryAfter, 10)
    return isNaN(seconds) ? undefined : seconds * 1000
  }
  return undefined
}
