import { useState, useCallback, useEffect } from 'react'
import { useAuthStore } from '@/stores/auth.store'
import { logger } from '@/lib/logger'

interface UseA2ATokenRefreshReturn {
  token: string | undefined
  isRefreshing: boolean
  refresh: () => Promise<void>
  error: string | undefined
  clearError: () => void
}

export function useA2ATokenRefresh(): UseA2ATokenRefreshReturn {
  const getAuthHeader = useAuthStore((state) => state.getAuthHeader)
  const accessToken = useAuthStore((state) => state.accessToken)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [error, setError] = useState<string | undefined>(undefined)

  const extractToken = useCallback((header: string | undefined): string | undefined => {
    if (!header) return undefined
    return header.replace('Bearer ', '')
  }, [])

  const authHeader = getAuthHeader()
  const token = extractToken(authHeader) || extractToken(accessToken ? `Bearer ${accessToken}` : undefined)

  const refresh = useCallback(async () => {
    try {
      setIsRefreshing(true)
      setError(undefined)

      const { authService } = await import('@/services/auth.service')
      const { token: refreshedToken, error: refreshError } = await authService.generateAnonymousToken()

      if (refreshError || !refreshedToken) {
        const message = refreshError || 'Token refresh failed'
        setError(String(message))
        logger.error('Token refresh failed', new Error(String(message)), 'useA2ATokenRefresh')
        throw new Error(String(message))
      }

      logger.debug('A2A token refreshed', undefined, 'useA2ATokenRefresh')
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Token refresh failed'
      setError(message)
      logger.error('Token refresh error', err, 'useA2ATokenRefresh')
      throw err
    } finally {
      setIsRefreshing(false)
    }
  }, [])

  const clearError = useCallback(() => {
    setError(undefined)
  }, [])

  useEffect(() => {
    if (!token) {
      logger.debug('No A2A token available', undefined, 'useA2ATokenRefresh')
    }
  }, [token])

  return { token, isRefreshing, refresh, error, clearError }
}
