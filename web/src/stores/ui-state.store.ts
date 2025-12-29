import { create } from 'zustand'
import type { ExecutionStep } from '@/types/execution'
import type { EphemeralArtifact } from '@/types/artifact'
import {
  createExecutionId,
  createTaskId,
  createArtifactId,
  type TaskId,
  type ContextId,
  type ExecutionId,
  type ArtifactId,
  type MessageId,
} from '@/types/core/brand'

export interface ToolExecution {
  id: ExecutionId
  toolName: string
  serverName: string
  status: 'pending' | 'executing' | 'completed' | 'error'
  artifactId?: ArtifactId
  error?: string
  timestamp: number
  parameters?: Record<string, unknown>
  executionTime?: number
}

export interface InputRequest {
  taskId: TaskId
  messageId: MessageId
  message?: unknown
  timestamp: Date
}

export interface AuthRequest {
  taskId: TaskId
  messageId: MessageId
  message?: unknown
  timestamp: Date
}

interface UIStateStore {
  activeStreamingTaskId: TaskId | undefined

  stepsById: Record<ExecutionId, ExecutionStep>
  stepIdsByTask: Record<TaskId, readonly ExecutionId[]>
  stepIdsByContext: Record<ContextId, readonly ExecutionId[]>

  toolExecutionsById: Record<ExecutionId, ToolExecution>
  toolExecutionIdsByTask: Record<TaskId, readonly ExecutionId[]>

  inputRequestsByTask: Record<TaskId, InputRequest>
  authRequestsByTask: Record<TaskId, AuthRequest>

  ephemeralArtifact: EphemeralArtifact | undefined

  setStreaming: (taskId: TaskId | undefined) => void

  addStep: (step: ExecutionStep, contextId?: ContextId) => void
  addSteps: (steps: ExecutionStep[], contextId?: ContextId) => void
  updateStep: (stepId: ExecutionId, updates: Partial<ExecutionStep>) => void
  getStepsByTask: (taskId: TaskId) => ExecutionStep[]
  getStepsByContext: (contextId: ContextId) => ExecutionStep[]
  clearStepsByTask: (taskId: TaskId) => void

  addToolExecution: (taskId: TaskId, execution: ToolExecution) => void
  completeToolExecution: (executionId: ExecutionId, artifactId?: ArtifactId) => void
  failToolExecution: (executionId: ExecutionId, error: string) => void
  removeToolExecution: (executionId: ExecutionId) => void
  getAllToolExecutions: () => ToolExecution[]
  getToolExecutionsByTask: (taskId: TaskId) => ToolExecution[]
  getActiveToolExecutions: () => ToolExecution[]
  findToolExecutionByArtifactId: (artifactId: ArtifactId) => ToolExecution | undefined
  getToolExecutionById: (executionId: ExecutionId) => ToolExecution | undefined
  completeToolExecutionByArtifact: (artifact: { artifactId?: string; metadata?: { tool_name?: string } }) => void

  registerInputRequest: (request: InputRequest) => void
  resolveInputRequest: (taskId: TaskId) => void
  registerAuthRequest: (request: AuthRequest) => void
  resolveAuthRequest: (taskId: TaskId) => void
  getFirstPendingInputRequest: () => InputRequest | undefined
  getFirstPendingAuthRequest: () => AuthRequest | undefined
  hasPendingInputRequests: () => boolean
  hasPendingAuthRequests: () => boolean

  setEphemeralArtifact: (artifact: EphemeralArtifact | undefined) => void

  reset: () => void
  resetForContext: (contextId: ContextId) => void
}

const initialState = {
  activeStreamingTaskId: undefined,
  stepsById: {},
  stepIdsByTask: {},
  stepIdsByContext: {},
  toolExecutionsById: {},
  toolExecutionIdsByTask: {},
  inputRequestsByTask: {},
  authRequestsByTask: {},
  ephemeralArtifact: undefined,
}

export const useUIStateStore = create<UIStateStore>()((set, get) => ({
  ...initialState,

  setStreaming: (taskId) => set({ activeStreamingTaskId: taskId }),

  addStep: (step, contextId) => {
    set((state) => {
      const existing = state.stepsById[step.stepId]

      if (existing) {
        return {
          stepsById: {
            ...state.stepsById,
            [step.stepId]: {
              ...existing,
              ...step,
              errorMessage: step.errorMessage ?? existing.errorMessage,
              durationMs: step.durationMs ?? existing.durationMs,
              content: step.content ?? existing.content,
            },
          },
        }
      }

      const taskSteps = state.stepIdsByTask[step.taskId] || []
      const newStepIdsByTask = taskSteps.includes(step.stepId)
        ? state.stepIdsByTask
        : { ...state.stepIdsByTask, [step.taskId]: [...taskSteps, step.stepId] }

      const newStepIdsByContext = contextId
        ? (() => {
            const contextSteps = state.stepIdsByContext[contextId] || []
            return contextSteps.includes(step.stepId)
              ? state.stepIdsByContext
              : { ...state.stepIdsByContext, [contextId]: [...contextSteps, step.stepId] }
          })()
        : state.stepIdsByContext

      return {
        stepsById: { ...state.stepsById, [step.stepId]: step },
        stepIdsByTask: newStepIdsByTask,
        stepIdsByContext: newStepIdsByContext,
      }
    })
  },

  addSteps: (steps, contextId) => {
    steps.forEach((step) => get().addStep(step, contextId))
  },

  updateStep: (stepId, updates) => {
    set((state) => {
      const existing = state.stepsById[stepId]
      if (!existing) return state
      return {
        stepsById: {
          ...state.stepsById,
          [stepId]: { ...existing, ...updates },
        },
      }
    })
  },

  getStepsByTask: (taskId) => {
    const state = get()
    const stepIds = state.stepIdsByTask[taskId] || []
    return stepIds
      .map((id) => state.stepsById[id])
      .filter((step): step is ExecutionStep => !!step)
      .sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime())
  },

  getStepsByContext: (contextId) => {
    const state = get()
    const stepIds = state.stepIdsByContext[contextId] || []
    return stepIds
      .map((id) => state.stepsById[id])
      .filter((step): step is ExecutionStep => !!step)
      .sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime())
  },

  clearStepsByTask: (taskId) => {
    set((state) => {
      const stepIds = state.stepIdsByTask[taskId] || []
      const newStepsById = { ...state.stepsById }
      stepIds.forEach((id) => delete newStepsById[id])
      const newStepIdsByTask = { ...state.stepIdsByTask }
      delete newStepIdsByTask[taskId]
      return { stepsById: newStepsById, stepIdsByTask: newStepIdsByTask }
    })
  },

  addToolExecution: (taskId, execution) => {
    set((state) => {
      const taskExecutions = state.toolExecutionIdsByTask[taskId] || []
      return {
        toolExecutionsById: {
          ...state.toolExecutionsById,
          [execution.id]: execution,
        },
        toolExecutionIdsByTask: {
          ...state.toolExecutionIdsByTask,
          [taskId]: [...taskExecutions, execution.id],
        },
      }
    })
  },

  completeToolExecution: (executionId, artifactId) => {
    set((state) => {
      const execution = state.toolExecutionsById[executionId]
      if (!execution) return state
      return {
        toolExecutionsById: {
          ...state.toolExecutionsById,
          [executionId]: {
            ...execution,
            status: 'completed' as const,
            artifactId,
            executionTime: Date.now() - execution.timestamp,
          },
        },
      }
    })
  },

  failToolExecution: (executionId, error) => {
    set((state) => {
      const execution = state.toolExecutionsById[executionId]
      if (!execution) return state
      return {
        toolExecutionsById: {
          ...state.toolExecutionsById,
          [executionId]: {
            ...execution,
            status: 'error' as const,
            error,
            executionTime: Date.now() - execution.timestamp,
          },
        },
      }
    })
  },

  getToolExecutionsByTask: (taskId) => {
    const state = get()
    const executionIds = state.toolExecutionIdsByTask[taskId] || []
    return executionIds
      .map((id) => state.toolExecutionsById[id])
      .filter((exec): exec is ToolExecution => !!exec)
  },

  getActiveToolExecutions: () => {
    const state = get()
    return Object.values(state.toolExecutionsById).filter(
      (exec) => exec.status === 'pending' || exec.status === 'executing'
    )
  },

  findToolExecutionByArtifactId: (artifactId) => {
    const state = get()
    return Object.values(state.toolExecutionsById).find(
      (exec) => exec.artifactId === artifactId
    )
  },

  getToolExecutionById: (executionId) => {
    return get().toolExecutionsById[executionId]
  },

  completeToolExecutionByArtifact: (artifact) => {
    const toolName = artifact.metadata?.tool_name
    if (!toolName) return

    set((state) => {
      const newToolExecutionsById = { ...state.toolExecutionsById }
      for (const [executionId, execution] of Object.entries(newToolExecutionsById)) {
        if (execution.toolName === toolName && execution.status === 'executing') {
          newToolExecutionsById[createExecutionId(executionId)] = {
            ...execution,
            status: 'completed' as const,
            artifactId: artifact.artifactId ? createArtifactId(artifact.artifactId) : undefined,
            executionTime: Date.now() - execution.timestamp,
          }
        }
      }
      return { toolExecutionsById: newToolExecutionsById }
    })
  },

  removeToolExecution: (executionId) => {
    set((state) => {
      const { [executionId]: removed, ...remainingExecutions } = state.toolExecutionsById
      if (!removed) return state

      const newToolExecutionIdsByTask = { ...state.toolExecutionIdsByTask }
      for (const taskId of Object.keys(newToolExecutionIdsByTask)) {
        const typedTaskId = createTaskId(taskId)
        newToolExecutionIdsByTask[typedTaskId] = newToolExecutionIdsByTask[typedTaskId].filter(
          (id: ExecutionId) => id !== executionId
        )
      }

      return {
        toolExecutionsById: remainingExecutions,
        toolExecutionIdsByTask: newToolExecutionIdsByTask,
      }
    })
  },

  getAllToolExecutions: () => {
    return Object.values(get().toolExecutionsById)
  },

  registerInputRequest: (request) => {
    set((state) => ({
      inputRequestsByTask: {
        ...state.inputRequestsByTask,
        [request.taskId]: request,
      },
    }))
  },

  resolveInputRequest: (taskId) => {
    set((state) => {
      const newInputRequests = { ...state.inputRequestsByTask }
      delete newInputRequests[taskId]
      return { inputRequestsByTask: newInputRequests }
    })
  },

  registerAuthRequest: (request) => {
    set((state) => ({
      authRequestsByTask: {
        ...state.authRequestsByTask,
        [request.taskId]: request,
      },
    }))
  },

  resolveAuthRequest: (taskId) => {
    set((state) => {
      const newAuthRequests = { ...state.authRequestsByTask }
      delete newAuthRequests[taskId]
      return { authRequestsByTask: newAuthRequests }
    })
  },

  getFirstPendingInputRequest: () => {
    const requests = Object.values(get().inputRequestsByTask)
    return requests.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime())[0]
  },

  getFirstPendingAuthRequest: () => {
    const requests = Object.values(get().authRequestsByTask)
    return requests.sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime())[0]
  },

  hasPendingInputRequests: () => Object.keys(get().inputRequestsByTask).length > 0,
  hasPendingAuthRequests: () => Object.keys(get().authRequestsByTask).length > 0,

  setEphemeralArtifact: (artifact) => set({ ephemeralArtifact: artifact }),

  reset: () => set(initialState),

  resetForContext: (contextId) => {
    set((state) => {
      const stepIds = state.stepIdsByContext[contextId] || []
      const newStepsById = { ...state.stepsById }
      stepIds.forEach((id) => delete newStepsById[id])
      const newStepIdsByContext = { ...state.stepIdsByContext }
      delete newStepIdsByContext[contextId]

      return {
        stepsById: newStepsById,
        stepIdsByContext: newStepIdsByContext,
      }
    })
  },
}))

export const uiStateSelectors = {
  isTaskStreaming: (state: UIStateStore, taskId: TaskId): boolean =>
    state.activeStreamingTaskId === taskId,

  hasStepsForTask: (state: UIStateStore, taskId: TaskId): boolean =>
    (state.stepIdsByTask[taskId]?.length ?? 0) > 0,

  getStepCount: (state: UIStateStore, taskId: TaskId): number =>
    state.stepIdsByTask[taskId]?.length ?? 0,
}
