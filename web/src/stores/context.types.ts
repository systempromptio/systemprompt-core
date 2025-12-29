import type { ContextId } from '@/types/core/brand'

export interface Conversation {
  id: ContextId
  name: string
  createdAt: Date
  updatedAt: Date
  messageCount: number
}

interface ContextEventBase {
  type: string
  context_id?: string
  timestamp: string
}

export interface ContextCreatedEvent extends ContextEventBase {
  type: 'context_created'
  context: {
    context_id: string
    name: string
    created_at: string
    updated_at: string
  }
}

export interface ContextUpdatedEvent extends ContextEventBase {
  type: 'context_updated'
  name: string
}

export interface ContextDeletedEvent extends ContextEventBase {
  type: 'context_deleted'
}

export interface CurrentAgentEvent extends ContextEventBase {
  type: 'current_agent'
  agent_name: string | null
}

export type ContextStateEvent =
  | ContextCreatedEvent
  | ContextUpdatedEvent
  | ContextDeletedEvent
  | CurrentAgentEvent
  | (ContextEventBase & { type: string })

export type SSEStatus = 'connected' | 'connecting' | 'disconnected' | 'error'

export interface ContextSnapshotItem {
  context_id: string
  name: string
  created_at: string
  updated_at: string
  message_count: number
}

export interface ContextStore {
  conversations: Map<ContextId, Conversation>
  currentContextId: ContextId | typeof CONTEXT_STATE.LOADING
  isLoading: boolean
  isCreatingInitialContext: boolean
  error: string | undefined
  hasReceivedSnapshot: boolean
  contextAgents: Map<ContextId, string>

  sseStatus: SSEStatus
  sseError: string | undefined

  conversationList: () => Conversation[]
  getCurrentConversation: () => Conversation | undefined
  createConversation: (name?: string) => Promise<void>
  switchConversation: (id: ContextId) => void
  renameConversation: (id: ContextId, name: string) => Promise<void>
  deleteConversation: (id: ContextId) => Promise<void>
  updateMessageCount: (id: ContextId) => void
  clearError: () => void

  setSSEStatus: (status: SSEStatus) => void
  setSSEError: (error: string | undefined) => void
  handleSnapshot: (contexts: ContextSnapshotItem[]) => void
  handleStateEvent: (event: ContextStateEvent) => void
}

export const CONTEXT_STATE = {
  LOADING: '__CONTEXT_LOADING__',
} as const
