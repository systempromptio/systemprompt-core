import { useEffect, useRef, useState, useMemo } from 'react'
import { useTaskStore } from '@/stores/task.store'
import { useContextStore, CONTEXT_STATE } from '@/stores/context.store'
import { useArtifactStore } from '@/stores/artifact.store'
import { useUIStateStore } from '@/stores/ui-state.store'
import { useAuthStore } from '@/stores/auth.store'
import { toArtifact } from '@/types/artifact'
import { extractAndStoreSkill } from '@/lib/utils/extractArtifactSkills'
import { logger } from '@/lib/logger'
import type { Task } from '@/types/task'
import { type TaskId, createTaskId } from '@/types/core/brand'

interface UseTaskLoaderReturn {
  tasks: Task[]
  isLoading: boolean
  contextId: string | undefined
}

export function useTaskLoader(): UseTaskLoaderReturn {
  const currentContextId = useContextStore((s) => s.currentContextId)
  const updateMessageCount = useContextStore((s) => s.updateMessageCount)
  const isLoadingContexts = useContextStore((s) => s.isLoading)
  const getAuthHeader = useAuthStore((s) => s.getAuthHeader)

  const byContext = useTaskStore((s) => s.byContext)
  const byId = useTaskStore((s) => s.byId)

  const [isLoadingTasks, setIsLoadingTasks] = useState(false)
  const abortControllerRef = useRef<AbortController | undefined>(undefined)

  const tasks = useMemo(() => {
    if (!currentContextId || currentContextId === CONTEXT_STATE.LOADING) return []

    const taskIds = byContext[currentContextId] || []
    return taskIds
      .map((id: TaskId) => byId[id])
      .filter((task): task is Task => task !== undefined)
      .filter((task) => task.metadata?.task_type !== 'mcp_execution')
      .sort((a, b) => {
        const aTime = new Date(a.metadata?.created_at || 0).getTime()
        const bTime = new Date(b.metadata?.created_at || 0).getTime()
        return aTime - bTime
      })
  }, [byContext, byId, currentContextId])

  useEffect(() => {
    if (!currentContextId || currentContextId === CONTEXT_STATE.LOADING) return

    const validContextId = currentContextId
    const unsubscribe = useArtifactStore.subscribe((state, prevState) => {
      const currentCtxArtifacts = Object.values(state.byId).filter(
        (a) => a.metadata.context_id === validContextId
      )
      const prevCtxArtifacts = Object.values(prevState.byId).filter(
        (a) => a.metadata.context_id === validContextId
      )

      if (currentCtxArtifacts.length > prevCtxArtifacts.length) {
        updateMessageCount(validContextId)
      }
    })

    return () => unsubscribe()
  }, [currentContextId, updateMessageCount])

  useEffect(() => {
    const authHeader = getAuthHeader()

    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
    }

    if (!currentContextId || !authHeader || currentContextId === CONTEXT_STATE.LOADING) {
      return
    }

    const controller = new AbortController()
    abortControllerRef.current = controller

    setIsLoadingTasks(true)

    const fetchAndProcessTasks = async () => {
      await useTaskStore.getState().fetchTasksByContext(currentContextId, authHeader)

      if (controller.signal.aborted) return

      const taskStore = useTaskStore.getState()
      const taskIds = taskStore.byContext[currentContextId] || []

      taskIds.forEach((taskId) => {
        const task = taskStore.byId[taskId]
        if (!task) return

        const executionSteps = task.metadata?.executionSteps
        if (executionSteps && Array.isArray(executionSteps) && executionSteps.length > 0) {
          const stepsWithTaskId = executionSteps.map((step) => ({
            ...step,
            taskId: createTaskId(step.taskId || task.id),
          }))
          useUIStateStore.getState().addSteps(stepsWithTaskId, currentContextId)
        }

        if (task.artifacts && task.artifacts.length > 0) {
          task.artifacts.forEach((artifact) => {
            try {
              const validated = toArtifact(artifact)
              useArtifactStore.getState().addArtifact(
                validated,
                createTaskId(task.id),
                currentContextId
              )
              extractAndStoreSkill(validated, currentContextId, createTaskId(task.id))
            } catch (e) {
              logger.warn('Skipping invalid artifact', e, 'useTaskLoader')
            }
          })
        }
      })
    }

    fetchAndProcessTasks()
      .catch((error) => {
        if (error.name === 'AbortError' || controller.signal.aborted) return
        logger.error('Error loading tasks', error, 'useTaskLoader')
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setIsLoadingTasks(false)
        }
      })

    return () => {
      if (abortControllerRef.current) {
        abortControllerRef.current.abort()
      }
    }
  }, [currentContextId, getAuthHeader])

  return {
    tasks,
    isLoading: isLoadingTasks || isLoadingContexts,
    contextId: currentContextId,
  }
}
