const RETRY_CONFIG = {
  maxAttempts: 3,
  baseDelayMs: 1000,
  maxDelayMs: 10000,
  backoffMultiplier: 2,
} as const

const RETRYABLE_STATUS_CODES = [429, 500, 502, 503, 504] as const

type RetryableStatusCode = (typeof RETRYABLE_STATUS_CODES)[number]

function isRetryableStatus(status: number): status is RetryableStatusCode {
  return RETRYABLE_STATUS_CODES.includes(status as RetryableStatusCode)
}

function calculateBackoffDelay(attempt: number): number {
  const delay = RETRY_CONFIG.baseDelayMs * Math.pow(RETRY_CONFIG.backoffMultiplier, attempt)
  return Math.min(delay, RETRY_CONFIG.maxDelayMs)
}

function shouldRetry(attempt: number, status: number): boolean {
  return attempt < RETRY_CONFIG.maxAttempts && isRetryableStatus(status)
}

export {
  RETRY_CONFIG,
  RETRYABLE_STATUS_CODES,
  isRetryableStatus,
  calculateBackoffDelay,
  shouldRetry,
}
export type { RetryableStatusCode }
