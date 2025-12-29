import type { MessageRoleValue } from './event-types'
import type { JsonPatchOperation } from './json-patch'

export interface RunStartedPayload {
  threadId: string
  runId: string
  input?: unknown
}

export interface RunFinishedPayload {
  threadId: string
  runId: string
  result?: unknown
}

export interface RunErrorPayload {
  message: string
  code?: string
}

export interface StepStartedPayload {
  stepName: string
}

export interface StepFinishedPayload {
  stepName: string
}

export interface TextMessageStartPayload {
  messageId: string
  role: MessageRoleValue
}

export interface TextMessageContentPayload {
  messageId: string
  delta: string
}

export interface TextMessageEndPayload {
  messageId: string
}

export interface ToolCallStartPayload {
  toolCallId: string
  toolCallName: string
  parentMessageId?: string
}

export interface ToolCallArgsPayload {
  toolCallId: string
  delta: string
}

export interface ToolCallEndPayload {
  toolCallId: string
}

export interface ToolCallResultPayload {
  messageId: string
  toolCallId: string
  content: unknown
  role: MessageRoleValue
}

export interface StateSnapshotPayload {
  snapshot: unknown
}

export interface StateDeltaPayload {
  delta: JsonPatchOperation[]
}

export interface MessagesSnapshotPayload {
  messages: unknown[]
}

export interface CustomPayload {
  name: string
  value: unknown
}

export interface ArtifactCustomValue {
  artifact: unknown
  taskId: string
  contextId: string
}

export interface ExecutionStepCustomValue {
  step: {
    stepId: string
    taskId: string
    status: 'pending' | 'in_progress' | 'completed' | 'failed'
    startedAt: string
    completedAt?: string
    durationMs?: number
    errorMessage?: string
    content: {
      type: 'understanding' | 'planning' | 'skill_usage' | 'tool_execution' | 'completion'
      reasoning?: string
      planned_tools?: Array<{ tool_name: string; arguments: unknown }>
      skill_id?: string
      skill_name?: string
      tool_name?: string
      tool_arguments?: unknown
      tool_result?: unknown
    }
  }
  contextId: string
}

export interface SkillLoadedCustomValue {
  skillId: string
  skillName: string
  description?: string
  taskId?: string
}

export interface RunStartedCustomValue {
  task: {
    id: string
    contextId: string
    status: {
      state: string
      message?: string | null
      timestamp: string
    }
    history: Array<{
      role: string
      parts: Array<{ kind: string; text?: string }>
      messageId: string
      taskId: string | null
      contextId: string
      kind: string
      metadata?: unknown | null
      extensions?: unknown | null
      referenceTaskIds?: string[] | null
    }>
    artifacts?: unknown | null
    metadata?: {
      task_type?: string
      agent_name?: string
      created_at?: string
      [key: string]: unknown
    } | null
    kind: string
  }
  threadId: string
  runId: string
}

export interface TaskCompletedCustomValue {
  task: {
    id: string
    contextId: string
    status: {
      state: string
      message?: string | null
      timestamp: string
    }
    history: Array<{
      role: string
      parts: Array<{ kind: string; text?: string }>
      messageId: string
      taskId: string
      contextId: string
      kind: string
      metadata?: unknown | null
      extensions?: unknown | null
      referenceTaskIds?: string[] | null
    }>
    artifacts?: unknown | null
    metadata?: {
      task_type?: string
      agent_name?: string
      created_at?: string
      updated_at?: string
      started_at?: string
      completed_at?: string
      execution_time_ms?: number
      executionSteps?: Array<{
        stepId: string
        taskId: string
        status: string
        startedAt: string
        completedAt?: string
        durationMs?: number
        content: {
          type: string
        }
      }>
      [key: string]: unknown
    } | null
    kind: string
  }
  artifacts?: unknown[] | null
  executionSteps?: Array<{
    stepId: string
    taskId: string
    status: string
    startedAt: string
    completedAt?: string
    durationMs?: number
    content: {
      type: string
    }
  }>
}
