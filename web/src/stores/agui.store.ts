import { create } from 'zustand'

import type { JsonPatchOperation } from '@/types/agui'
import { applyJsonPatch } from '@/types/agui'

interface PendingMessage {
  role: string
  content: string
}

interface ActiveToolCall {
  name: string
  arguments: string
}

interface AgUiState {
  snapshot: Record<string, unknown>
  pendingMessages: Map<string, PendingMessage>
  activeToolCalls: Map<string, ActiveToolCall>
  currentRunId: string | undefined
  currentThreadId: string | undefined
}

interface AgUiActions {
  setSnapshot: (snapshot: Record<string, unknown>) => void
  applyDelta: (operations: JsonPatchOperation[]) => void
  startRun: (threadId: string, runId: string) => void
  endRun: () => void
  startMessage: (messageId: string, role: string) => void
  appendMessageContent: (messageId: string, delta: string) => void
  endMessage: (messageId: string) => PendingMessage | undefined
  startToolCall: (toolCallId: string, toolName: string) => void
  appendToolArgs: (toolCallId: string, delta: string) => void
  endToolCall: (toolCallId: string) => ActiveToolCall | undefined
  reset: () => void
}

type AgUiStore = AgUiState & AgUiActions

const initialState: AgUiState = {
  snapshot: {},
  pendingMessages: new Map(),
  activeToolCalls: new Map(),
  currentRunId: undefined,
  currentThreadId: undefined,
}

export const useAgUiStore = create<AgUiStore>()((set, get) => ({
  ...initialState,

  setSnapshot: (snapshot) => set({ snapshot }),

  applyDelta: (operations) => {
    set((state) => ({
      snapshot: applyJsonPatch(state.snapshot, operations),
    }))
  },

  startRun: (threadId, runId) => {
    set({ currentThreadId: threadId, currentRunId: runId })
  },

  endRun: () => {
    set({ currentRunId: undefined, currentThreadId: undefined })
  },

  startMessage: (messageId, role) => {
    set((state) => {
      const updated = new Map(state.pendingMessages)
      updated.set(messageId, { role, content: '' })
      return { pendingMessages: updated }
    })
  },

  appendMessageContent: (messageId, delta) => {
    set((state) => {
      const updated = new Map(state.pendingMessages)
      const existing = updated.get(messageId)
      if (existing) {
        updated.set(messageId, { ...existing, content: existing.content + delta })
      }
      return { pendingMessages: updated }
    })
  },

  endMessage: (messageId) => {
    const message = get().pendingMessages.get(messageId)
    if (message) {
      set((state) => {
        const updated = new Map(state.pendingMessages)
        updated.delete(messageId)
        return { pendingMessages: updated }
      })
    }
    return message
  },

  startToolCall: (toolCallId, toolName) => {
    set((state) => {
      const updated = new Map(state.activeToolCalls)
      updated.set(toolCallId, { name: toolName, arguments: '' })
      return { activeToolCalls: updated }
    })
  },

  appendToolArgs: (toolCallId, delta) => {
    set((state) => {
      const updated = new Map(state.activeToolCalls)
      const existing = updated.get(toolCallId)
      if (existing) {
        updated.set(toolCallId, { ...existing, arguments: existing.arguments + delta })
      }
      return { activeToolCalls: updated }
    })
  },

  endToolCall: (toolCallId) => {
    const toolCall = get().activeToolCalls.get(toolCallId)
    if (toolCall) {
      set((state) => {
        const updated = new Map(state.activeToolCalls)
        updated.delete(toolCallId)
        return { activeToolCalls: updated }
      })
    }
    return toolCall
  },

  reset: () => set(initialState),
}))
