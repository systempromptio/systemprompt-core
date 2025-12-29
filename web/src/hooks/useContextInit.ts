/**
 * Hook for automatic initialization of default conversation context.
 *
 * Ensures that a default conversation is created when the user is authenticated
 * but no conversations exist yet. Runs automatically on auth state changes and
 * snapshot receipt.
 *
 * This hook requires no parameters and is typically mounted at the app root level
 * to initialize the context layer on authentication.
 *
 * @throws {Error} When creating default context fails (logged but not thrown)
 *
 * @example
 * ```typescript
 * function App() {
 *   // Mount at app root to ensure context is initialized
 *   useContextInit()
 *
 *   const { conversations } = useContextStore()
 *
 *   return (
 *     <div>
 *       <ConversationList conversations={conversations} />
 *       <ChatView />
 *     </div>
 *   )
 * }
 * ```
 */

import { useEffect, useRef } from 'react'
import { useAuthStore } from '@/stores/auth.store'
import { useContextStore, CONTEXT_STATE } from '@/stores/context.store'
import { logger } from '@/lib/logger'

export function useContextInit() {
  const accessToken = useAuthStore((state) => state.accessToken)
  const isTokenValid = useAuthStore((state) => state.isTokenValid)
  const currentContextId = useContextStore((state) => state.currentContextId)
  const conversations = useContextStore((state) => state.conversations)
  const hasReceivedSnapshot = useContextStore((state) => state.hasReceivedSnapshot)
  const isCreatingInitialContext = useContextStore((state) => state.isCreatingInitialContext)
  const createConversation = useContextStore((state) => state.createConversation)
  const isCreatingContextLocal = useRef(false)

  useEffect(() => {
    const ensureContext = async () => {
      if (isCreatingContextLocal.current) {
        return
      }

      const hasValidToken = accessToken && isTokenValid()
      const isInLoadingState = currentContextId === CONTEXT_STATE.LOADING
      const hasNoContexts = conversations.size === 0
      const shouldCreateDefaultContext = hasValidToken && isInLoadingState && hasReceivedSnapshot && hasNoContexts && isCreatingInitialContext

      if (shouldCreateDefaultContext) {
        isCreatingContextLocal.current = true

        try {
          logger.debug('Creating default context', undefined, 'useContextInit')
          await createConversation('Default Conversation')
          logger.debug('Default context created successfully', undefined, 'useContextInit')
        } catch (error) {
          logger.error('Failed to create default context', error, 'useContextInit')
        } finally {
          isCreatingContextLocal.current = false
        }
      }
    }

    ensureContext()
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accessToken, isTokenValid, currentContextId, conversations.size, hasReceivedSnapshot, isCreatingInitialContext])
}
