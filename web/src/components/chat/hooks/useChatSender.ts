import { useState, useCallback } from 'react'
import { useA2AClient } from '@/hooks/useA2AClient'
import { useTaskStore } from '@/stores/task.store'
import { useArtifactStore } from '@/stores/artifact.store'
import { useUIStateStore } from '@/stores/ui-state.store'
import { isTaskEvent, isStatusUpdateEvent } from '../helpers/typeGuards'
import type { Task } from '@/types/task'
import type { Artifact } from '@/types/artifact'
import type { Task as A2ATask } from '@a2a-js/sdk'
import { toTask } from '@/types/task'
import { toArtifact } from '@/types/artifact'
import { createTaskId, createContextId } from '@/types/core/brand'

interface UseChatSenderReturn {
  sendMessage: (text: string, files?: File[]) => Promise<void>
  isSending: boolean
  error: string | null
  clearError: () => void
}

export function useChatSender(): UseChatSenderReturn {
  const { streamMessage, sendMessage: sendMessageApi } = useA2AClient()

  const [isSending, setIsSending] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const clearError = useCallback(() => {
    setError(null)
  }, [])

  const sendMessage = useCallback(
    async (text: string, files?: File[]) => {
      try {
        setIsSending(true)
        setError(null)

        if (!files?.length && streamMessage) {
          try {
            for await (const event of streamMessage(text)) {
              if (isStatusUpdateEvent(event)) {
                const state = event.status.state
                if (state === 'failed' || state === 'rejected' || state === 'canceled') {
                  useUIStateStore.getState().clearStepsByTask(createTaskId(event.taskId))
                  useUIStateStore.getState().setStreaming(undefined)

                  const existingTask = useTaskStore.getState().byId[createTaskId(event.taskId)]
                  if (existingTask) {
                    useTaskStore.getState().updateTask({
                      ...existingTask,
                      status: {
                        state: state as 'failed' | 'rejected' | 'canceled',
                        message: event.status.message ? {
                          role: 'agent',
                          parts: [{ kind: 'text' as const, text: event.status.message }],
                          messageId: '',
                          kind: 'message',
                          contextId: event.contextId || existingTask.contextId || '',
                        } : undefined,
                        timestamp: new Date().toISOString(),
                      },
                    })
                  }
                }
                continue
              }

              if (isTaskEvent(event)) {
                const rawTask = event as A2ATask

                let validatedTask: Task
                try {
                  validatedTask = toTask(rawTask)
                } catch {
                  continue
                }

                useTaskStore.getState().updateTask(validatedTask)

                const taskState = validatedTask.status?.state
                const isTerminalState = taskState === 'completed' || taskState === 'failed' || taskState === 'rejected' || taskState === 'canceled'
                if (isTerminalState) {
                  useUIStateStore.getState().clearStepsByTask(createTaskId(validatedTask.id))
                  useUIStateStore.getState().setStreaming(undefined)
                }

                if (validatedTask.artifacts && validatedTask.artifacts.length > 0) {
                  const validatedArtifacts = validatedTask.artifacts
                    .map((a) => {
                      try {
                        return toArtifact(a)
                      } catch {
                        return null
                      }
                    })
                    .filter((a): a is Artifact => a !== null)

                  validatedArtifacts.forEach((artifact) => {
                    useArtifactStore.getState().addArtifact(
                      artifact,
                      createTaskId(validatedTask.id),
                      createContextId(validatedTask.contextId || '')
                    )
                  })
                }
              }
            }
          } catch (streamError: unknown) {
            useUIStateStore.getState().setStreaming(undefined)

            const errorMessage = streamError instanceof Error ? streamError.message : String(streamError)
            const isTokenExpired =
              errorMessage.toLowerCase().includes('expired') ||
              errorMessage.toLowerCase().includes('token has expired')
            const isPermissionError =
              errorMessage.includes('401') ||
              errorMessage.includes('403') ||
              errorMessage.includes('Unauthorized') ||
              errorMessage.includes('Forbidden')

            if (isTokenExpired) {
              setError('Your session has expired. Please refresh the page to continue.')
              return
            }

            if (isPermissionError) {
              setError('Permission denied. You may not have sufficient permissions to access this agent.')
              return
            }

            throw streamError
          }
        } else {
          await sendMessageApi?.(text, files)
        }
      } catch (err: unknown) {
        useUIStateStore.getState().setStreaming(undefined)

        const errorMessage = err instanceof Error ? err.message : String(err)
        const isTokenExpired =
          errorMessage.toLowerCase().includes('expired') ||
          errorMessage.toLowerCase().includes('token has expired')
        const isPermissionError =
          errorMessage.includes('401') ||
          errorMessage.includes('403') ||
          errorMessage.includes('Unauthorized') ||
          errorMessage.includes('Forbidden')

        if (isTokenExpired) {
          setError('Your session has expired. Please refresh the page to continue.')
        } else if (isPermissionError) {
          setError('Permission denied. You may not have sufficient permissions to access this agent.')
        } else {
          setError(err instanceof Error ? err.message : 'Failed to send message')
        }
      } finally {
        setIsSending(false)
      }
    },
    [streamMessage, sendMessageApi]
  )

  return { sendMessage, isSending, error, clearError }
}
