import { useEffect, useState, useCallback } from 'react'
import { A2AService, getA2AClient } from '@/lib/a2a/client'
import { useAgentStore } from '@/stores/agent.store'
import { useAuthStore } from '@/stores/auth.store'
import { logger } from '@/lib/logger'
import { createAgentUrl } from '@/types/core/brand'
import type { AgentCard } from '@a2a-js/sdk'

const MAX_RETRIES = 3

export function useA2AClientInitialization() {
  const [client, setClient] = useState<A2AService | undefined>(undefined)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<Error | undefined>(undefined)
  const [retrying, setRetrying] = useState(false)

  const selectedAgentUrl = useAgentStore((state) => state.selectedAgentUrl)
  const selectedAgent = useAgentStore((state) => state.selectedAgent)
  const agents = useAgentStore((state) => state.agents)
  const selectAgent = useAgentStore((state) => state.selectAgent)
  const accessToken = useAuthStore((state) => state.accessToken)

  const initializeClientInternal = useCallback(async (
    agentUrl: string,
    agentCard: AgentCard | undefined,
    authHeader: string | undefined
  ): Promise<A2AService> => {
    const service = getA2AClient(agentUrl, authHeader)

    if (agentCard) {
      try {
        await service.initialize(agentCard)
        logger.debug('Successfully initialized with existing card', undefined, 'useA2AClientInitialization')
        return service
      } catch (initError) {
        logger.error('Failed to initialize with existing card', initError, 'useA2AClientInitialization')
        throw initError
      }
    }

    if (agentUrl.includes('/api/v1/agents/')) {
      const proxyError = new Error('Agent card not found for proxy URL. Please refresh agent discovery.')
      logger.error('Agent card not found for proxy URL', proxyError, 'useA2AClientInitialization')
      throw proxyError
    }

    try {
      const fetchedCard = await service.initialize()
      const card: AgentCard = {
        ...fetchedCard,
        url: agentUrl
      }
      selectAgent(createAgentUrl(agentUrl), card)
      logger.debug('Successfully initialized from well-known', undefined, 'useA2AClientInitialization')
      return service
    } catch (wellKnownError) {
      logger.error('Failed to initialize from well-known', wellKnownError, 'useA2AClientInitialization')
      throw wellKnownError
    }
  }, [selectAgent])

  useEffect(() => {
    if (!selectedAgentUrl) {
      setClient(undefined)
      return
    }

    const authHeader = accessToken ? `Bearer ${accessToken}` : undefined
    setLoading(true)
    setError(undefined)

    let agentCard = selectedAgent

    if (!agentCard && agents.length > 0) {
      agentCard = agents.find(a => a.url === selectedAgentUrl)
      if (agentCard && selectedAgentUrl) {
        selectAgent(createAgentUrl(selectedAgentUrl), agentCard)
      }
    }

    const attemptInitialization = async (attempt: number) => {
      try {
        const initializedClient = await initializeClientInternal(selectedAgentUrl, agentCard, authHeader)
        setClient(initializedClient)
        setLoading(false)
        setRetrying(false)
        setError(undefined)
      } catch (attemptError) {
        if (attempt < MAX_RETRIES) {
          const nextAttempt = attempt + 1
          const retryDelay = Math.min(1000 * Math.pow(2, nextAttempt), 5000)
          logger.debug('Retrying agent connection', { attempt: nextAttempt, maxRetries: MAX_RETRIES }, 'useA2AClientInitialization')

          setRetrying(true)
          setError(new Error(`Connection failed. Retrying (${nextAttempt}/${MAX_RETRIES})...`))

          await new Promise(resolve => setTimeout(resolve, retryDelay))
          await attemptInitialization(nextAttempt)
        } else {
          logger.error('Max retries reached, initialization failed', attemptError, 'useA2AClientInitialization')
          setError(new Error('Failed to connect to agent. Please refresh the page or try again later.'))
          setClient(undefined)
          setLoading(false)
          setRetrying(false)
        }
      }
    }

    attemptInitialization(0)
  }, [selectedAgentUrl, selectedAgent, agents, selectAgent, accessToken, initializeClientInternal])

  const retryConnection = useCallback(() => {
    if (!selectedAgentUrl) return

    setError(undefined)
    setClient(undefined)
    setLoading(true)

    const authHeader = accessToken ? `Bearer ${accessToken}` : undefined
    let agentCard = selectedAgent

    if (!agentCard && agents.length > 0) {
      agentCard = agents.find(a => a.url === selectedAgentUrl)
    }

    const attemptInitialization = async () => {
      try {
        const initializedClient = await initializeClientInternal(selectedAgentUrl, agentCard, authHeader)
        setClient(initializedClient)
        setLoading(false)
        setRetrying(false)
        setError(undefined)
        logger.debug('Retry successful, client reinitialized', undefined, 'useA2AClientInitialization')
      } catch (retryError) {
        const errorToSet = retryError instanceof Error ? retryError : new Error(typeof retryError === 'string' ? retryError : 'Connection failed')
        setError(errorToSet)
        setClient(undefined)
        setLoading(false)
        setRetrying(false)
      }
    }

    attemptInitialization()
  }, [selectedAgentUrl, selectedAgent, agents, accessToken, initializeClientInternal])

  return {
    client,
    loading,
    error,
    retrying,
    retryConnection
  }
}
