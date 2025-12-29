import { useState, useCallback, useEffect } from 'react'
import { A2AService, getA2AClient } from '@/lib/a2a/client'
import { useAgentStore } from '@/stores/agent.store'
import { useAuthStore } from '@/stores/auth.store'
import { useContextStore } from '@/stores/context.store'
import { logger } from '@/lib/logger'
import { createAgentUrl } from '@/types/core/brand'
import type { AgentCard } from '@a2a-js/sdk'

const MAX_RETRIES = 3

interface InitializationState {
  client: A2AService | null
  isInitializing: boolean
  isReady: boolean
  error: Error | null
  retryCount: number
  isRetrying: boolean
}

interface UseA2AInitializerReturn extends InitializationState {
  retryConnection: () => void
  disconnect: () => void
}

export function useA2AInitializer(): UseA2AInitializerReturn {
  const selectedAgentUrl = useAgentStore((state) => state.selectedAgentUrl)
  const selectedAgent = useAgentStore((state) => state.selectedAgent)
  const agents = useAgentStore((state) => state.agents)
  const selectAgent = useAgentStore((state) => state.selectAgent)
  const accessToken = useAuthStore((state) => state.accessToken)
  const currentContextId = useContextStore((state) => state.currentContextId)

  const [state, setState] = useState<InitializationState>({
    client: null,
    isInitializing: false,
    isReady: false,
    error: null,
    retryCount: 0,
    isRetrying: false,
  })

  const initializeClient = useCallback(
    async (agentUrl: string, agentCard: AgentCard | undefined, authHeader: string | undefined): Promise<A2AService> => {
      const service = getA2AClient(agentUrl, authHeader)

      if (agentCard) {
        await service.initialize(agentCard)
        logger.debug('Successfully initialized with existing card', undefined, 'useA2AInitializer')
        return service
      } else if (agentUrl.includes('/api/v1/agents/')) {
        throw new Error('Agent card not found for proxy URL. Please refresh agent discovery.')
      } else {
        const fetchedCard = await service.initialize()
        const card: AgentCard = {
          ...fetchedCard,
          url: agentUrl,
        }
        selectAgent(createAgentUrl(agentUrl), card)
        logger.debug('Successfully initialized from well-known', undefined, 'useA2AInitializer')
        return service
      }
    },
    [selectAgent]
  )

  const attemptInitialization = useCallback(
    async (isManualRetry: boolean = false) => {
      if (!selectedAgentUrl) {
        setState((prev) => ({
          ...prev,
          client: null,
          retryCount: 0,
        }))
        return
      }

      const authHeader = accessToken ? `Bearer ${accessToken}` : undefined
      setState((prev) => ({
        ...prev,
        isInitializing: !isManualRetry,
        error: null,
      }))

      let agentCard = selectedAgent

      if (!agentCard && agents.length > 0) {
        agentCard = agents.find((a) => a.url === selectedAgentUrl)
        if (agentCard && selectedAgentUrl) {
          selectAgent(createAgentUrl(selectedAgentUrl), agentCard)
        }
      }

      try {
        const client = await initializeClient(selectedAgentUrl, agentCard, authHeader)
        setState((prev) => ({
          ...prev,
          client,
          isInitializing: false,
          isReady: true,
          isRetrying: false,
          error: null,
          retryCount: 0,
        }))
      } catch (err) {
        setState((prev) => {
          const nextRetryCount = prev.retryCount + 1

          if (nextRetryCount < MAX_RETRIES) {
            const retryDelay = Math.min(1000 * Math.pow(2, nextRetryCount), 5000)
            logger.debug('Retrying agent connection', { attempt: nextRetryCount, maxRetries: MAX_RETRIES }, 'useA2AInitializer')

            setTimeout(() => {
              attemptInitialization(false)
            }, retryDelay)

            return {
              ...prev,
              isRetrying: true,
              isInitializing: false,
              error: new Error(`Connection failed. Retrying (${nextRetryCount}/${MAX_RETRIES})...`),
              retryCount: nextRetryCount,
            }
          } else {
            logger.error('Max retries reached, initialization failed', err, 'useA2AInitializer')
            return {
              ...prev,
              client: null,
              isInitializing: false,
              isReady: false,
              isRetrying: false,
              error: new Error('Failed to connect to agent. Please refresh the page or try again later.'),
              retryCount: nextRetryCount,
            }
          }
        })
      }
    },
    [selectedAgentUrl, selectedAgent, agents, selectAgent, accessToken, initializeClient]
  )

  useEffect(() => {
    attemptInitialization(false)
  }, [selectedAgentUrl, currentContextId, attemptInitialization])

  const retryConnection = useCallback(() => {
    setState((prev) => ({
      ...prev,
      retryCount: 0,
      error: null,
      client: null,
      isInitializing: true,
    }))
    attemptInitialization(true)
  }, [attemptInitialization])

  const disconnect = useCallback(() => {
    if (state.client && 'disconnect' in state.client && typeof state.client.disconnect === 'function') {
      state.client.disconnect()
    }
    setState((prev) => ({
      ...prev,
      client: null,
      isInitializing: false,
      isReady: false,
      isRetrying: false,
      error: null,
      retryCount: 0,
    }))
    logger.debug('A2A client disconnected', undefined, 'useA2AInitializer')
  }, [state.client])

  useEffect(() => {
    return () => {
      disconnect()
    }
  }, [disconnect])

  return { ...state, retryConnection, disconnect }
}
