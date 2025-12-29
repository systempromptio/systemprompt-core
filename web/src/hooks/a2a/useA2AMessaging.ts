import { useState, useCallback } from 'react'
import { useContextStore } from '@/stores/context.store'
import { logger } from '@/lib/logger'
import type { A2AService } from '@/lib/a2a/client'
import type { Task, Message, TaskStatusUpdateEvent, TaskArtifactUpdateEvent } from '@a2a-js/sdk'

type A2AStreamEventData = Message | Task | TaskStatusUpdateEvent | TaskArtifactUpdateEvent

interface MessagingState {
  isSending: boolean
  error: Error | undefined
}

interface UseA2AMessagingReturn extends MessagingState {
  sendMessage: (text: string, files?: File[]) => Promise<Task | Message>
  streamMessage: (text: string) => AsyncGenerator<A2AStreamEventData, void, unknown>
  getTask: (taskId: string) => Promise<Task>
  cancelTask: (taskId: string) => Promise<Task>
  clearError: () => void
}

export function useA2AMessaging(client: A2AService | undefined): UseA2AMessagingReturn {
  const currentContextId = useContextStore((state) => state.currentContextId)
  const hasReceivedSnapshot = useContextStore((state) => state.hasReceivedSnapshot)
  const [state, setState] = useState<MessagingState>({
    isSending: false,
    error: undefined,
  })

  const clearError = useCallback(() => {
    setState((prev) => ({ ...prev, error: undefined }))
  }, [])

  const ensureReady = useCallback((): A2AService => {
    if (!client) {
      const clientError = new Error('Client not initialized')
      setState((prev) => ({ ...prev, error: clientError }))
      throw clientError
    }

    if (!hasReceivedSnapshot) {
      const contextError = new Error('Cannot send message: Context not initialized. Please wait for contexts to load.')
      setState((prev) => ({ ...prev, error: contextError }))
      throw contextError
    }

    return client
  }, [client, hasReceivedSnapshot])

  const sendMessage = useCallback(
    async (text: string, files?: File[]): Promise<Task | Message> => {
      const readyClient = ensureReady()

      try {
        setState((prev) => ({ ...prev, isSending: true, error: undefined }))

        const response = await readyClient.sendMessage(text, files, currentContextId)
        logger.debug('A2A message sent', { length: text.length }, 'useA2AMessaging')

        return response
      } catch (sendError) {
        const errorToSet = sendError instanceof Error ? sendError : new Error('Failed to send message')
        setState((prev) => ({ ...prev, error: errorToSet }))
        logger.error('A2A message send failed', sendError, 'useA2AMessaging')
        throw errorToSet
      } finally {
        setState((prev) => ({ ...prev, isSending: false }))
      }
    },
    [currentContextId, ensureReady]
  )

  const streamMessage = useCallback(
    async function* (text: string): AsyncGenerator<A2AStreamEventData, void, unknown> {
      const readyClient = ensureReady()

      try {
        setState((prev) => ({ ...prev, isSending: true, error: undefined }))

        for await (const event of readyClient.streamMessage(text, currentContextId)) {
          yield event
        }

        logger.debug('A2A streaming message completed', { length: text.length }, 'useA2AMessaging')
      } catch (streamError) {
        const errorToSet = streamError instanceof Error ? streamError : new Error('Failed to stream message')
        setState((prev) => ({ ...prev, error: errorToSet }))
        logger.error('A2A streaming failed', streamError, 'useA2AMessaging')
        throw errorToSet
      } finally {
        setState((prev) => ({ ...prev, isSending: false }))
      }
    },
    [currentContextId, ensureReady]
  )

  const getTask = useCallback(
    async (taskId: string): Promise<Task> => {
      const readyClient = ensureReady()

      try {
        setState((prev) => ({ ...prev, error: undefined }))
        const task = await readyClient.getTask(taskId)
        logger.debug('A2A task fetched', { taskId }, 'useA2AMessaging')
        return task
      } catch (fetchError) {
        const errorToSet = fetchError instanceof Error ? fetchError : new Error('Failed to get task')
        setState((prev) => ({ ...prev, error: errorToSet }))
        logger.error('A2A get task failed', fetchError, 'useA2AMessaging')
        throw errorToSet
      }
    },
    [ensureReady]
  )

  const cancelTask = useCallback(
    async (taskId: string): Promise<Task> => {
      const readyClient = ensureReady()

      try {
        setState((prev) => ({ ...prev, error: undefined }))
        const task = await readyClient.cancelTask(taskId)
        logger.debug('A2A task cancelled', { taskId }, 'useA2AMessaging')
        return task
      } catch (cancelError) {
        const errorToSet = cancelError instanceof Error ? cancelError : new Error('Failed to cancel task')
        setState((prev) => ({ ...prev, error: errorToSet }))
        logger.error('A2A cancel task failed', cancelError, 'useA2AMessaging')
        throw errorToSet
      }
    },
    [ensureReady]
  )

  return {
    ...state,
    sendMessage,
    streamMessage,
    getTask,
    cancelTask,
    clearError,
  }
}
