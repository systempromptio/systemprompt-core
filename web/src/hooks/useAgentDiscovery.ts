import { useEffect, useCallback, useRef } from 'react'
import { A2AClient } from '@a2a-js/sdk/client'
import type { AgentCard } from '@a2a-js/sdk'
import { useAgentStore } from '@/stores/agent.store'
import { useContextStore, CONTEXT_STATE } from '@/stores/context.store'
import { useAuthStore } from '@/stores/auth.store'
import { useSkillStore } from '@/stores/skill.store'
import { logger } from '@/lib/logger'
import { createAgentUrl } from '@/types/core/brand'

export interface AgentEndpoint {
  url: string
  name: string
  description?: string
}

const fetchAgentsFromAPI = async (): Promise<AgentCard[]> => {
  try {
    const authHeader = useAuthStore.getState().getAuthHeader()
    if (!authHeader) {
      logger.error('Missing authentication', new Error('No JWT token available'), 'useAgentDiscovery')
      throw new Error('Missing authentication')
    }

    logger.debug('Fetching agents from registry API', undefined, 'useAgentDiscovery')
    const response = await fetch('/api/v1/agents/registry', {
      headers: {
        'Authorization': authHeader,
      },
    })
    if (!response.ok) {
      logger.error('Failed to fetch agents', new Error(`API returned ${response.status}`), 'useAgentDiscovery')
      throw new Error(`API returned ${response.status}`)
    }
    const data = await response.json()

    if (data.data && Array.isArray(data.data)) {
      const agents = data.data as AgentCard[]
      logger.debug('Successfully loaded agents', { count: agents.length }, 'useAgentDiscovery')
      return agents
    }

    logger.warn('No agents found in response', undefined, 'useAgentDiscovery')
    return []
  } catch (error) {
    logger.error('Failed to fetch agents from registry API', error, 'useAgentDiscovery')
    return []
  }
}

export function useAgentDiscovery() {
  const {
    agents,
    selectedAgent,
    setAgents,
    addAgent,
    selectAgent,
    setLoading,
    setError
  } = useAgentStore()
  const hasAttemptedLoad = useRef(false)

  const discoverAgent = useCallback(
    async (endpoint: AgentEndpoint): Promise<AgentCard | null> => {
      try {
        let client: A2AClient
        const isProxyPath = endpoint.url.includes('/server/')

        try {
          const cardUrl = `${endpoint.url}/.well-known/agent-card.json`
          client = await A2AClient.fromCardUrl(cardUrl)
        } catch {
          if (!isProxyPath) {
            const fallbackUrl = `${endpoint.url}/api/agents/card`
            client = await A2AClient.fromCardUrl(fallbackUrl)
          } else {
            throw new Error('Failed to fetch agent card through proxy')
          }
        }

        const card = await client.getAgentCard()
        const agentCard: AgentCard = {
          ...card,
          url: endpoint.url
        }
        logger.debug('Successfully discovered agent', { name: card.name }, 'useAgentDiscovery')
        return agentCard
      } catch (error) {
        logger.error(`Failed to discover agent at endpoint`, error, 'useAgentDiscovery')
        return null
      }
    },
    []
  )

  const discoverAllAgents = useCallback(async () => {
    hasAttemptedLoad.current = true
    setLoading(true)
    setError(undefined)

    try {
      const apiAgents = await fetchAgentsFromAPI()

      if (apiAgents.length > 0) {
        logger.debug('Setting agents in store', { count: apiAgents.length }, 'useAgentDiscovery')
        setAgents(apiAgents)

        const allSkills = apiAgents.flatMap(agent => {
          if ('skills' in agent && Array.isArray(agent.skills)) {
            return agent.skills
          }
          return []
        })
        if (allSkills.length > 0) {
          logger.debug('Loading skills from agent cards', { count: allSkills.length }, 'useAgentDiscovery')
          useSkillStore.getState().loadSkills(allSkills)
        }

        const contextStore = useContextStore.getState()
        const currentContextId = contextStore.currentContextId
        const assignedAgentName = currentContextId !== CONTEXT_STATE.LOADING
          ? contextStore.contextAgents.get(currentContextId)
          : undefined

        if (assignedAgentName) {
          const matchingAgent = apiAgents.find((agent: AgentCard) =>
            agent.name.toLowerCase() === assignedAgentName.toLowerCase()
          )
          if (matchingAgent) {
            selectAgent(createAgentUrl(matchingAgent.url), matchingAgent)
          } else {
            logger.warn('Assigned agent not found', { agentName: assignedAgentName }, 'useAgentDiscovery')
          }
        } else if (!selectedAgent && apiAgents.length > 0) {
          const defaultAgent = apiAgents.find((agent: AgentCard) => {
            const serviceStatusExt = agent.capabilities?.extensions?.find(
              (ext: { uri: string }) => ext.uri === 'systemprompt:service-status'
            )
            return serviceStatusExt?.params?.default === true
          })
          const agentToSelect = defaultAgent || apiAgents[0]
          selectAgent(createAgentUrl(agentToSelect.url), agentToSelect)
        }
      } else {
        logger.warn('No agents found in registry', undefined, 'useAgentDiscovery')
        setError('No agents found in registry')
      }
    } catch (err) {
      logger.error('Agent discovery error', err, 'useAgentDiscovery')
      setError('Failed to discover agents from registry')
    } finally {
      setLoading(false)
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setAgents, selectAgent, setLoading, setError])

  const addCustomAgent = useCallback(
    async (url: string): Promise<boolean> => {
      setLoading(true)
      setError(undefined)

      try {
        const agent = await discoverAgent({
          url,
          name: 'Custom Agent',
          description: 'User-added agent',
        })

        if (agent) {
          logger.debug('Custom agent added successfully', { name: agent.name }, 'useAgentDiscovery')
          addAgent(agent)
          return true
        } else {
          logger.error('Failed to discover custom agent', new Error(`Failed at URL: ${url}`), 'useAgentDiscovery')
          setError(`Failed to connect to agent at ${url}`)
          return false
        }
      } catch (err) {
        logger.error('Error connecting to agent', err, 'useAgentDiscovery')
        setError(`Error connecting to agent: ${err}`)
        return false
      } finally {
        setLoading(false)
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [discoverAgent, addAgent]
  )

  useEffect(() => {
    if (agents.length === 0 && !hasAttemptedLoad.current) {
      discoverAllAgents()
    }
  }, [agents.length, discoverAllAgents])

  return {
    refresh: discoverAllAgents,
    addCustomAgent,
  }
}