import { useCallback, useState } from 'react'
import { A2AService } from '@/lib/a2a/client'
import { useAuthStore } from '@/stores/auth.store'
import { logger } from '@/lib/logger'

export function useA2AClientReady(client: A2AService | undefined, onRetry: () => void) {
  const [error, setError] = useState<Error | undefined>(undefined)

  const ensureClientReady = useCallback(async (): Promise<boolean> => {
    const { isTokenValid, userType, setAnonymousAuth, clearAuth } = useAuthStore.getState()
    const { authService } = await import('@/services/auth.service')

    if (!isTokenValid()) {
      if (userType === 'anon') {
        try {
          const { token, error: authError } = await authService.generateAnonymousToken()

          if (authError || !token) {
            logger.error('Failed to refresh anonymous token', new Error(String(authError)), 'useA2AClientReady')
            clearAuth()
            setError(new Error('Session expired. Please refresh the page.'))
            return false
          }

          setAnonymousAuth(
            token.access_token,
            token.user_id,
            token.session_id,
            token.expires_in
          )
          logger.debug('Token refreshed, client will reinitialize automatically', undefined, 'useA2AClientReady')
          return false
        } catch (refreshError) {
          logger.error('Error refreshing token', refreshError, 'useA2AClientReady')
          clearAuth()
          setError(new Error('Session expired. Please refresh the page.'))
          return false
        }
      } else {
        logger.debug('Authenticated user token expired, clearing auth', undefined, 'useA2AClientReady')
        clearAuth()
        setError(new Error('Session expired. Please log in again.'))
        return false
      }
    }

    if (!client) {
      setError(new Error('Client not initialized. Retrying connection...'))
      onRetry()
      return false
    }

    setError(undefined)
    return true
  }, [client, onRetry])

  return { ensureClientReady, error, setError }
}
