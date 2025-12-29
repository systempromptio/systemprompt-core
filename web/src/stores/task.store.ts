import { create } from 'zustand'
import type { TaskState } from '@a2a-js/sdk'
import type { Task } from '@/types/task'
import { tasksService } from '@/services/tasks.service'
import { shouldReplaceItem } from '@/utils/store-helpers'
import { type TaskId, type ContextId, createTaskId, createContextId } from '@/types/core/brand'

const ensureTaskIdInArray = (taskId: TaskId, allIds: readonly TaskId[]): readonly TaskId[] => {
  return allIds.includes(taskId) ? allIds : [...allIds, taskId]
}

const normalizeTasksIntoState = (
  state: { byId: Record<TaskId, Task>; allIds: readonly TaskId[] },
  tasks: Task[]
) => {
  const newById: Record<TaskId, Task> = { ...state.byId }
  const newAllIds: TaskId[] = [...state.allIds]

  tasks.forEach((task) => {
    const taskId = createTaskId(task.id)
    const existing = state.byId[taskId]
    const mergedTask = {
      ...task,
      history: (task.history?.length ?? 0) > 0
        ? task.history
        : existing?.history ?? []
    }
    newById[taskId] = mergedTask
    if (!newAllIds.includes(taskId)) {
      newAllIds.push(taskId)
    }
  })

  return { newById, newAllIds }
}

interface TaskStore {
  byId: Record<TaskId, Task>
  allIds: readonly TaskId[]
  byContext: Readonly<Record<ContextId, readonly TaskId[]>>
  isLoading: boolean
  error: string | undefined

  fetchTasksByContext: (contextId: ContextId, authToken: string | undefined) => Promise<void>
  fetchTask: (taskId: TaskId, authToken: string | undefined) => Promise<void>
  fetchTasks: (authToken: string | undefined, status?: string, limit?: number) => Promise<void>
  updateTask: (task: Task) => void
  clearError: () => void
  getTasksByContext: (contextId: ContextId) => Task[]
  getTasksByStatus: (status: TaskState) => Task[]
  reset: () => void
}

export const useTaskStore = create<TaskStore>()((set, get) => ({
  byId: {},
  allIds: [],
  byContext: {},
  isLoading: false,
  error: undefined,

  fetchTasksByContext: async (contextId, authToken) => {
    set({ isLoading: true, error: undefined })

    const result = await tasksService.listTasksByContext(contextId, authToken)

    if (!result.ok) {
      const errorMessage = result.error.kind === 'network' && result.error.status === 404
        ? `Context ${contextId} not found`
        : 'message' in result.error
          ? result.error.message
          : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const tasks = [...result.value]
    set((state) => {
      const { newById, newAllIds } = normalizeTasksIntoState(state, tasks)
      const taskIds: TaskId[] = tasks.map(task => createTaskId(task.id))

      return {
        byId: newById,
        allIds: newAllIds,
        byContext: { ...state.byContext, [contextId]: taskIds },
        isLoading: false,
      }
    })
  },

  fetchTask: async (taskId, authToken) => {
    set({ isLoading: true, error: undefined })

    const result = await tasksService.getTask(taskId, authToken)

    if (!result.ok) {
      const errorMessage = result.error.kind === 'network' && result.error.status === 404
        ? `Task ${taskId} not found`
        : 'message' in result.error
          ? result.error.message
          : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const task = result.value
    const fetchedTaskId = createTaskId(task.id)
    set((state) => ({
      byId: { ...state.byId, [fetchedTaskId]: task },
      allIds: ensureTaskIdInArray(fetchedTaskId, state.allIds),
      isLoading: false,
    }))
  },

  fetchTasks: async (authToken, status?, limit?) => {
    set({ isLoading: true, error: undefined })

    const result = await tasksService.listTasks(authToken, status, limit)

    if (!result.ok) {
      const errorMessage = 'message' in result.error
        ? result.error.message
        : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const tasks = [...result.value]
    set((state) => {
      const { newById, newAllIds } = normalizeTasksIntoState(state, tasks)

      return {
        byId: newById,
        allIds: newAllIds,
        isLoading: false,
      }
    })
  },

  updateTask: (task) => {
    set((state) => {
      const taskId = createTaskId(task.id)
      const contextId = createContextId(task.contextId)
      const existing = state.byId[taskId]

      if (!shouldReplaceItem(task.metadata, existing?.metadata)) {
        return state
      }

      const newByContext: Record<ContextId, readonly TaskId[]> = { ...state.byContext }
      const existingTaskIds = state.byContext[contextId] || []
      if (!existingTaskIds.includes(taskId)) {
        newByContext[contextId] = [...existingTaskIds, taskId]
      }

      const mergedTask = {
        ...task,
        history: (task.history?.length ?? 0) > 0
          ? task.history
          : existing?.history ?? []
      }

      return {
        byId: { ...state.byId, [taskId]: mergedTask },
        allIds: ensureTaskIdInArray(taskId, state.allIds),
        byContext: newByContext,
      }
    })
  },

  clearError: () => set({ error: undefined }),

  getTasksByContext: (contextId) => {
    const state = get()
    const taskIds = state.byContext[contextId] || []
    return taskIds
      .map((id) => state.byId[id])
      .filter((task): task is Task => task !== undefined)
  },

  getTasksByStatus: (status) => {
    const state = get()
    return state.allIds
      .map(id => state.byId[id])
      .filter((task) => task.status.state === status)
  },

  reset: () => {
    set({
      byId: {},
      allIds: [],
      byContext: {},
      isLoading: false,
      error: undefined,
    })
  },
}))

export const taskSelectors = {
  getTaskById: (state: TaskStore, id: TaskId): Task | undefined =>
    state.byId[id],

  getTasksByContextIds: (state: TaskStore, contextId: ContextId): readonly TaskId[] =>
    state.byContext[contextId] ?? [],

  getTaskCount: (state: TaskStore): number => state.allIds.length,

  isLoading: (state: TaskStore): boolean => state.isLoading,

  hasError: (state: TaskStore): boolean => state.error !== undefined,

  getError: (state: TaskStore): string | undefined => state.error,

  hasAnyTasks: (state: TaskStore): boolean => state.allIds.length > 0,
}
