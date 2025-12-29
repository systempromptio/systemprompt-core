import { useCallback } from 'react'
import { A2AService } from '@/lib/a2a/client'
import { useContextStore, getValidContextId, CONTEXT_STATE } from '@/stores/context.store'
import type { Task, Message, TaskStatusUpdateEvent, TaskArtifactUpdateEvent } from '@a2a-js/sdk'

type A2AStreamEventData = Message | Task | TaskStatusUpdateEvent | TaskArtifactUpdateEvent

function getContextErrorMessage(state: ReturnType<typeof useContextStore.getState>): string {
  if (!state.hasReceivedSnapshot) {
    return 'Cannot send message: Waiting for server connection. Please wait...'
  }
  if (state.isCreatingInitialContext) {
    return 'Cannot send message: Setting up conversation. Please wait...'
  }
  if (state.currentContextId === CONTEXT_STATE.LOADING) {
    return 'Cannot send message: Initializing conversation. Please wait...'
  }
  return 'Cannot send message: No valid context selected. Please select or create a conversation first.'
}

export function useA2AMessageOperations(
  client: A2AService | undefined,
  ensureClientReady: () => Promise<boolean>,
  onError: (error: Error | undefined) => void
) {
  const sendMessage = useCallback(
    async (text: string, files?: File[]): Promise<Task | Message> => {
      const isReady = await ensureClientReady()
      if (!isReady) {
        const error = new Error('A2A client is not ready. Please wait for initialization.')
        onError(error)
        throw error
      }

      const state = useContextStore.getState()
      const contextId = getValidContextId(state)
      if (!contextId) {
        const error = new Error(getContextErrorMessage(state))
        onError(error)
        throw error
      }

      if (!client) {
        const error = new Error('A2A client not available')
        onError(error)
        throw error
      }

      try {
        onError(undefined)
        const response = await client.sendMessage(text, files, contextId)
        return response
      } catch (caughtError) {
        const errorToSet = caughtError instanceof Error ? caughtError : new Error(typeof caughtError === 'string' ? caughtError : 'Failed to send message')
        onError(errorToSet)
        throw errorToSet
      }
    },
    [client, ensureClientReady, onError]
  )

  const streamMessage = useCallback(
    async function* (text: string, clientMessageId?: string): AsyncGenerator<A2AStreamEventData, void, unknown> {
      const isReady = await ensureClientReady()
      if (!isReady) {
        return
      }

      const state = useContextStore.getState()
      const contextId = getValidContextId(state)
      if (!contextId) {
        const error = new Error(getContextErrorMessage(state))
        onError(error)
        throw error
      }

      if (!client) {
        return
      }

      try {
        onError(undefined)
        for await (const event of client.streamMessage(text, contextId, clientMessageId)) {
          yield event
        }
      } catch (caughtError) {
        const errorToSet = caughtError instanceof Error ? caughtError : new Error(typeof caughtError === 'string' ? caughtError : 'Failed to stream message')
        onError(errorToSet)
        throw errorToSet
      }
    },
    [client, ensureClientReady, onError]
  )

  const getTask = useCallback(
    async (taskId: string): Promise<Task> => {
      const isReady = await ensureClientReady()
      if (!isReady) {
        const error = new Error('A2A client is not ready. Please wait for initialization.')
        onError(error)
        throw error
      }

      if (!client) {
        const error = new Error('A2A client not available')
        onError(error)
        throw error
      }

      try {
        onError(undefined)
        return await client.getTask(taskId)
      } catch (caughtError) {
        const errorToSet = caughtError instanceof Error ? caughtError : new Error(typeof caughtError === 'string' ? caughtError : 'Failed to get task')
        onError(errorToSet)
        throw errorToSet
      }
    },
    [client, ensureClientReady, onError]
  )

  const cancelTask = useCallback(
    async (taskId: string): Promise<Task> => {
      const isReady = await ensureClientReady()
      if (!isReady) {
        const error = new Error('A2A client is not ready. Please wait for initialization.')
        onError(error)
        throw error
      }

      if (!client) {
        const error = new Error('A2A client not available')
        onError(error)
        throw error
      }

      try {
        onError(undefined)
        return await client.cancelTask(taskId)
      } catch (caughtError) {
        const errorToSet = caughtError instanceof Error ? caughtError : new Error(typeof caughtError === 'string' ? caughtError : 'Failed to cancel task')
        onError(errorToSet)
        throw errorToSet
      }
    },
    [client, ensureClientReady, onError]
  )

  return {
    sendMessage,
    streamMessage,
    getTask,
    cancelTask
  }
}
