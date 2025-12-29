import { useCallback } from 'react'

import { logger } from '@/lib/logger'
import { useAgUiEventProcessor } from './agui/useAgUiEventProcessor'
import { isAgUiEvent } from '@/types/agui'
import { useContextStore } from '@/stores/context.store'
import type { ContextSnapshotItem } from '@/stores/context.types'
import { useTaskStore } from '@/stores/task.store'
import { useUIStateStore } from '@/stores/ui-state.store'
import { createTaskId } from '@/types/core/brand'
import type { TaskState } from '@a2a-js/sdk'

interface ContextEvent {
  protocol: 'agui' | 'a2a' | 'system'
  event: unknown
}

function isContextEvent(value: unknown): value is ContextEvent {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.protocol === 'string' &&
    ['agui', 'a2a', 'system'].includes(candidate.protocol) &&
    candidate.event !== undefined
  )
}

interface A2ATaskStatusUpdateEvent {
  type: 'TASK_STATUS_UPDATE'
  taskId: string
  contextId: string
  state: string
  message?: string
}

function isTaskStatusUpdateEvent(event: unknown): event is A2ATaskStatusUpdateEvent {
  if (typeof event !== 'object' || event === null) return false
  const e = event as Record<string, unknown>
  return e.type === 'TASK_STATUS_UPDATE' && typeof e.taskId === 'string'
}

function isContextSnapshotItem(item: unknown): item is ContextSnapshotItem {
  if (typeof item !== 'object' || item === null) return false
  const obj = item as Record<string, unknown>
  return (
    typeof obj.context_id === 'string' &&
    typeof obj.name === 'string' &&
    typeof obj.created_at === 'string' &&
    typeof obj.updated_at === 'string' &&
    typeof obj.message_count === 'number'
  )
}

function isContextSnapshotArray(items: unknown[]): items is ContextSnapshotItem[] {
  return items.every(isContextSnapshotItem)
}

function handleA2AEvent(event: unknown): void {
  if (isTaskStatusUpdateEvent(event)) {
    const { taskId, contextId, state, message } = event
    const existingTask = useTaskStore.getState().byId[createTaskId(taskId)]

    if (existingTask) {
      useTaskStore.getState().updateTask({
        ...existingTask,
        status: {
          state: state as TaskState,
          message: message ? {
            role: 'agent',
            parts: [{ kind: 'text' as const, text: message }],
            messageId: '',
            kind: 'message',
            contextId: contextId,
          } : undefined,
          timestamp: new Date().toISOString(),
        },
      })

      if (state === 'completed' || state === 'failed' || state === 'rejected' || state === 'canceled') {
        useUIStateStore.getState().clearStepsByTask(createTaskId(taskId))
        useUIStateStore.getState().setStreaming(undefined)
      }
    }

    logger.debug('A2A TASK_STATUS_UPDATE processed', { taskId, state }, 'useStreamEventProcessor')
  } else {
    logger.debug('A2A event received', { event }, 'useStreamEventProcessor')
  }
}

export function useStreamEventProcessor() {
  const { processEvent: processAgUiEvent } = useAgUiEventProcessor()

  const processEvent = useCallback(
    (_eventType: string | undefined, data: string) => {
      try {
        const parsed = JSON.parse(data)

        if (isContextEvent(parsed)) {
          if (parsed.protocol === 'agui' && isAgUiEvent(parsed.event)) {
            processAgUiEvent(JSON.stringify(parsed.event))
          } else if (parsed.protocol === 'system') {
            const event = parsed.event as { type?: string; contexts?: unknown[] }
            if (event.type === 'CONTEXTS_SNAPSHOT' && Array.isArray(event.contexts) && isContextSnapshotArray(event.contexts)) {
              useContextStore.getState().handleSnapshot(event.contexts)
            }
            logger.debug('System event received', { event: parsed.event }, 'useStreamEventProcessor')
          } else if (parsed.protocol === 'a2a') {
            handleA2AEvent(parsed.event)
          }
          return
        }

        if (isAgUiEvent(parsed)) {
          processAgUiEvent(data)
          return
        }

        logger.warn('Unknown event format', { data }, 'useStreamEventProcessor')
      } catch (error) {
        logger.error('Failed to process event', error, 'useStreamEventProcessor')
      }
    },
    [processAgUiEvent]
  )

  return { processEvent }
}
