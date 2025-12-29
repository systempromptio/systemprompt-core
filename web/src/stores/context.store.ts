import { create } from 'zustand'
import { contextsService } from '@/services/contexts.service'
import { useAuthStore } from './auth.store'
import { useAgentStore } from './agent.store'
import { logger } from '@/lib/logger'
import {
  getStorageKey,
  loadFromStorage,
  saveToStorage,
  setError,
  clearError,
} from './store-utilities'
import {
  CONTEXT_STATE,
  type Conversation,
  type ContextStore,
  type ContextCreatedEvent,
  type ContextUpdatedEvent,
  type ContextDeletedEvent,
  type CurrentAgentEvent,
  type ContextSnapshotItem,
} from './context.types'
import { type ContextId, createContextId, createAgentUrl } from '@/types/core/brand'
export { CONTEXT_STATE, type Conversation } from './context.types'

const INVALID_CONTEXT_IDS = ['undefined', 'null', '', '__CONTEXT_LOADING__']

function isValidContextIdForOperation(id: ContextId | string | undefined | null): id is ContextId {
  if (!id) return false
  if (typeof id !== 'string') return false
  if (INVALID_CONTEXT_IDS.includes(id)) return false
  return true
}

const getContextStorageKey = (userId?: string): string => {
  return getStorageKey('context', userId)
}

const loadPersistedContextId = (): ContextId | undefined => {
  const userId = useAuthStore.getState().userId
  const key = getContextStorageKey(userId || undefined)
  const stored = loadFromStorage(key)
  return stored ? createContextId(stored) : undefined
}

const persistContextId = (id: ContextId): void => {
  const userId = useAuthStore.getState().userId
  const key = getContextStorageKey(userId || undefined)
  saveToStorage(key, id)
}

const getAuthToken = (): string => {
  const { accessToken, isTokenValid } = useAuthStore.getState()

  if (!accessToken || !isTokenValid()) {
    throw new Error('Authentication required: No valid JWT token available')
  }

  return `Bearer ${accessToken}`
}

export function getValidContextId(state: { currentContextId: ContextId | typeof CONTEXT_STATE.LOADING; hasReceivedSnapshot: boolean; conversations: Map<ContextId, Conversation>; isCreatingInitialContext: boolean }): ContextId | undefined {
  if (state.currentContextId === CONTEXT_STATE.LOADING) {
    return undefined
  }
  if (!state.hasReceivedSnapshot) {
    return undefined
  }
  if (!state.conversations.has(state.currentContextId)) {
    return undefined
  }
  return state.currentContextId
}

export function useIsContextReady(): boolean {
  return useContextStore((state) => {
    if (!state.hasReceivedSnapshot) return false
    if (state.isCreatingInitialContext) return false
    if (state.currentContextId === CONTEXT_STATE.LOADING) return false
    return state.conversations.has(state.currentContextId)
  })
}

export function useIsContextInitializing(): boolean {
  return useContextStore((state) => {
    if (!state.hasReceivedSnapshot) return true
    if (state.isCreatingInitialContext) return true
    if (state.currentContextId === CONTEXT_STATE.LOADING && state.conversations.size === 0) return true
    return false
  })
}

export const useContextStore = create<ContextStore>()((set, get) => ({
  conversations: new Map(),
  currentContextId: CONTEXT_STATE.LOADING,
  isLoading: true,
  isCreatingInitialContext: false,
  error: undefined,
  hasReceivedSnapshot: false,
  contextAgents: new Map(),

  sseStatus: 'disconnected',
  sseError: undefined,

  conversationList: () => {
    const conversations = Array.from(get().conversations.values())
    return conversations.sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime())
  },

  getCurrentConversation: () => {
    const { currentContextId, conversations } = get()

    if (currentContextId === CONTEXT_STATE.LOADING) {
      return undefined
    }

    const conv = conversations.get(currentContextId)
    if (!conv) {
      return undefined
    }
    return conv
  },

  createConversation: async (name?: string) => {
    const generateDefaultName = () => {
      const existingConversations = Array.from(get().conversations.values())
      const conversationNumbers = existingConversations
        .map(c => {
          const match = c.name.match(/^Conversation (\d+)$/)
          return match ? parseInt(match[1], 10) : 0
        })
        .filter(n => n > 0)

      const maxNumber = conversationNumbers.length > 0
        ? Math.max(...conversationNumbers)
        : 0

      return `Conversation ${maxNumber + 1}`
    }

    const conversationName = (name && name.trim()) ? name.trim() : generateDefaultName()

    try {
      const authToken = getAuthToken()
      const result = await contextsService.createContext(conversationName, authToken)

      if (!result.ok) {
        const errorMessage = 'message' in result.error
          ? result.error.message
          : `Error: ${result.error.kind}`
        logger.error('Failed to create conversation', errorMessage, 'ContextStore')
        set({ isCreatingInitialContext: false, ...setError(`Failed to create conversation: ${errorMessage}`) })
        return
      }

      const context = result.value
      if (!context.context_id) {
        throw new Error('API returned context without context_id')
      }
      if (!context.name?.trim()) {
        throw new Error('API returned context without name')
      }
      const contextId = createContextId(context.context_id)
      const newConversation: Conversation = {
        id: contextId,
        name: context.name.trim(),
        createdAt: new Date(context.created_at),
        updatedAt: new Date(context.updated_at),
        messageCount: 0,
      }

      set((state) => {
        const updated = new Map(state.conversations)
        updated.set(contextId, newConversation)
        return {
          conversations: updated,
          currentContextId: contextId,
          isCreatingInitialContext: false,
          hasReceivedSnapshot: true,
          ...clearError(),
        }
      })

      persistContextId(contextId)
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error'
      logger.error('Error creating conversation', err, 'ContextStore')
      set({ isCreatingInitialContext: false, ...setError(`Failed to create conversation: ${message}`) })
    }
  },

  switchConversation: (id: ContextId) => {
    if (!isValidContextIdForOperation(id)) {
      logger.error('Cannot switch to invalid context ID', { id }, 'ContextStore')
      set(setError('Cannot switch to invalid context. Please select a valid conversation.'))
      return
    }
    const state = get()
    const conversations = state.conversations
    if (!conversations.has(id)) {
      logger.error('Cannot switch to non-existent context', { id }, 'ContextStore')
      set(setError(`Cannot switch to non-existent context: ${id}`))
      return
    }

    const agentName = state.contextAgents.get(id)
    if (agentName) {
      const agentStore = useAgentStore.getState()
      const matchingAgent = agentStore.agents.find(
        agent => agent.name === agentName
      )

      if (matchingAgent) {
        agentStore.selectAgent(createAgentUrl(matchingAgent.url), matchingAgent)
        logger.debug('Auto-selected agent for switched context', {
          contextId: id,
          agentName
        }, 'ContextStore')
      }
    }

    persistContextId(id)
    set({ currentContextId: id, ...clearError() })
  },

  renameConversation: async (id: ContextId, name: string) => {
    if (!isValidContextIdForOperation(id)) {
      logger.error('Cannot rename invalid context ID', { id }, 'ContextStore')
      set(setError('Cannot rename invalid context. Please select a valid conversation.'))
      return
    }
    const conversations = get().conversations
    if (!conversations.has(id)) {
      logger.error('Cannot rename non-existent context', { id }, 'ContextStore')
      set(setError(`Cannot rename non-existent context: ${id}`))
      return
    }

    const previousState = conversations.get(id)!

    set((state) => {
      const updated = new Map(state.conversations)
      const conv = updated.get(id)!
      updated.set(id, { ...conv, name, updatedAt: new Date() })
      return { conversations: updated, ...clearError() }
    })

    try {
      const authToken = getAuthToken()
      await contextsService.updateContext(id, name, authToken)
    } catch (error) {
      logger.error('Failed to rename conversation', error, 'ContextStore')
      set((state) => {
        const updated = new Map(state.conversations)
        updated.set(id, previousState)
        return {
          conversations: updated,
          ...setError('Failed to rename conversation')
        }
      })
    }
  },

  deleteConversation: async (id: ContextId) => {
    if (!isValidContextIdForOperation(id)) {
      logger.error('Cannot delete invalid context ID', { id }, 'ContextStore')
      set(setError('Cannot delete invalid context. Please select a valid conversation.'))
      return
    }
    const state = get()

    if (!state.conversations.has(id)) {
      logger.error('Cannot delete non-existent context', { id }, 'ContextStore')
      set(setError(`Cannot delete non-existent context: ${id}`))
      return
    }

    const updated = new Map(state.conversations)
    updated.delete(id)

    const remainingIds = Array.from(updated.keys())
    const newCurrentId = state.currentContextId === id
      ? remainingIds[0]
      : state.currentContextId

    if (!newCurrentId || newCurrentId === CONTEXT_STATE.LOADING) {
      logger.error('Cannot delete last context', undefined, 'ContextStore')
      set(setError('Cannot delete last context - system must always have at least one context'))
      return
    }

    const previousState = {
      conversations: state.conversations,
      currentContextId: state.currentContextId,
    }

    persistContextId(newCurrentId)

    set({
      conversations: updated,
      currentContextId: newCurrentId,
      ...clearError(),
    })

    try {
      const authToken = getAuthToken()
      await contextsService.deleteContext(id, authToken)
    } catch (error) {
      logger.error('Failed to delete conversation', error, 'ContextStore')
      if (previousState.currentContextId !== CONTEXT_STATE.LOADING) {
        persistContextId(previousState.currentContextId)
      }
      set({
        conversations: previousState.conversations,
        currentContextId: previousState.currentContextId,
        ...setError('Failed to delete conversation')
      })
    }
  },

  updateMessageCount: (id: ContextId) => {
    if (!isValidContextIdForOperation(id)) {
      logger.warn('updateMessageCount called with invalid context ID', { id }, 'ContextStore')
      return
    }
    const conversations = get().conversations
    if (!conversations.has(id)) {
      logger.warn('updateMessageCount called for non-existent context', { id }, 'ContextStore')
      return
    }

    set((state) => {
      const updated = new Map(state.conversations)
      const conv = updated.get(id)!
      updated.set(id, { ...conv, messageCount: conv.messageCount + 1, updatedAt: new Date() })
      return { conversations: updated }
    })
  },

  clearError: () => set({ error: undefined }),

  setSSEStatus: (status) => set({ sseStatus: status }),
  setSSEError: (error) => set({ sseError: error }),

  handleSnapshot: (contexts: ContextSnapshotItem[]) => {
    const state = get()
    logger.debug('Received snapshot', { count: contexts.length, existingCount: state.conversations.size }, 'ContextStore')

    if (contexts.length === 0) {
      const hasLocalConversations = state.conversations.size > 0
      if (hasLocalConversations) {
        logger.debug('Empty snapshot received but local contexts exist - preserving state', undefined, 'ContextStore')
        set({
          isLoading: false,
          hasReceivedSnapshot: true,
        })
        return
      }

      logger.debug('Empty snapshot - useContextInit will create default context', undefined, 'ContextStore')
      set({
        conversations: new Map(),
        currentContextId: CONTEXT_STATE.LOADING,
        isLoading: false,
        hasReceivedSnapshot: true,
        isCreatingInitialContext: true,
      })
      return
    }

    const conversationsArray = contexts.map((ctx): Conversation => {
      if (!ctx.context_id) {
        throw new Error('Snapshot contains context without context_id')
      }
      if (!ctx.name?.trim()) {
        throw new Error(`Snapshot contains context ${ctx.context_id} without name`)
      }
      return {
        id: createContextId(ctx.context_id),
        name: ctx.name.trim(),
        createdAt: new Date(ctx.created_at),
        updatedAt: new Date(ctx.updated_at),
        messageCount: ctx.message_count,
      }
    })

    const conversations = new Map(conversationsArray.map(c => [c.id, c]))
    const persistedId = loadPersistedContextId()

    const validPersistedId = persistedId && conversations.has(persistedId)
      ? persistedId
      : undefined

    const sortedArray = conversationsArray.sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime())
    const selectedId = validPersistedId || sortedArray[0].id

    persistContextId(selectedId)

    set({
      conversations,
      currentContextId: selectedId,
      isLoading: false,
      hasReceivedSnapshot: true,
      isCreatingInitialContext: false,
    })

    const agentName = get().contextAgents.get(selectedId)
    if (agentName) {
      const agentStore = useAgentStore.getState()
      const matchingAgent = agentStore.agents.find(
        agent => agent.name === agentName
      )

      if (matchingAgent) {
        agentStore.selectAgent(createAgentUrl(matchingAgent.url), matchingAgent)
        logger.debug('Auto-selected agent for initial context', {
          contextId: selectedId,
          agentName
        }, 'ContextStore')
      }
    }
  },

  handleStateEvent: (event) => {
    switch (event.type) {
      case 'context_created': {
        const createdEvent = event as ContextCreatedEvent
        if (!createdEvent.context?.context_id) {
          logger.error('context_created event missing context_id', event, 'ContextStore')
          return
        }
        if (!createdEvent.context.name?.trim()) {
          logger.error('context_created event missing name', event, 'ContextStore')
          return
        }

        const contextId = createContextId(createdEvent.context.context_id)
        const newConversation: Conversation = {
          id: contextId,
          name: createdEvent.context.name.trim(),
          createdAt: new Date(createdEvent.context.created_at),
          updatedAt: new Date(createdEvent.context.updated_at),
          messageCount: 0,
        }
        set((state) => {
          const updated = new Map(state.conversations)
          updated.set(contextId, newConversation)
          const shouldSwitchToNew = state.currentContextId === CONTEXT_STATE.LOADING || !state.conversations.has(state.currentContextId)
          const newCurrentId = shouldSwitchToNew ? contextId : state.currentContextId
          if (shouldSwitchToNew) {
            persistContextId(contextId)
          }
          return {
            conversations: updated,
            currentContextId: newCurrentId,
            isCreatingInitialContext: false,
            hasReceivedSnapshot: true,
          }
        })
        break
      }

      case 'context_updated': {
        const updatedEvent = event as ContextUpdatedEvent
        if (!updatedEvent.context_id) return

        const contextId = createContextId(updatedEvent.context_id)
        set((state) => {
          const updated = new Map(state.conversations)
          const conv = updated.get(contextId)
          if (conv) {
            updated.set(contextId, { ...conv, name: updatedEvent.name, updatedAt: new Date(updatedEvent.timestamp) })
          }
          return { conversations: updated }
        })
        break
      }

      case 'context_deleted': {
        const deletedEvent = event as ContextDeletedEvent
        if (!deletedEvent.context_id) return

        const contextId = createContextId(deletedEvent.context_id)
        set((state) => {
          const updated = new Map(state.conversations)
          updated.delete(contextId)

          const remainingKeys = Array.from(updated.keys())
          let newCurrentId: ContextId | typeof CONTEXT_STATE.LOADING = state.currentContextId
          let shouldPersist = false

          if (state.currentContextId === contextId) {
            newCurrentId = remainingKeys[0] || CONTEXT_STATE.LOADING
            shouldPersist = newCurrentId !== CONTEXT_STATE.LOADING
          } else if (state.currentContextId !== CONTEXT_STATE.LOADING && !updated.has(state.currentContextId)) {
            newCurrentId = remainingKeys[0] || CONTEXT_STATE.LOADING
            shouldPersist = newCurrentId !== CONTEXT_STATE.LOADING
          }

          if (shouldPersist && newCurrentId !== CONTEXT_STATE.LOADING) {
            persistContextId(newCurrentId)
          }

          const needsInitialContext = updated.size === 0

          return {
            conversations: updated,
            currentContextId: newCurrentId,
            isCreatingInitialContext: needsInitialContext,
          }
        })
        break
      }

      case 'current_agent': {
        const agentEvent = event as CurrentAgentEvent
        if (!agentEvent.context_id) return

        const contextId = createContextId(agentEvent.context_id)
        const agentName = agentEvent.agent_name

        set((state) => {
          const updated = new Map(state.contextAgents)
          if (agentName) {
            updated.set(contextId, agentName)
          } else {
            updated.delete(contextId)
          }
          return { contextAgents: updated }
        })

        if (agentName) {
          const agentStore = useAgentStore.getState()
          const matchingAgent = agentStore.agents.find(
            agent => agent.name.toLowerCase() === agentName.toLowerCase()
          )
          if (matchingAgent && get().currentContextId === contextId) {
            agentStore.selectAgent(createAgentUrl(matchingAgent.url), matchingAgent)
          }
        }
        break
      }
    }
  },
}))
