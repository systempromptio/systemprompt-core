import type { ExecutionStep, StepContent } from '@/types/execution'
import type { Task, TaskMetadata } from '@/types/task'
import type { Artifact } from '@/types/artifact'
import { hasArtifactId, validateArtifact } from '@/types/artifact'
import {
  createExecutionId,
  createTaskId,
  createSkillId,
} from '@/types/core/brand'
import type {
  ExecutionStepCustomValue,
  RunStartedCustomValue,
  TaskCompletedCustomValue,
} from '@/types/agui'

const VALID_TASK_TYPES = ['mcp_execution', 'agent_message'] as const
type TaskType = typeof VALID_TASK_TYPES[number]

function isValidTaskType(value: unknown): value is TaskType {
  return typeof value === 'string' && VALID_TASK_TYPES.includes(value as TaskType)
}

function getTaskType(value: unknown): TaskType {
  return isValidTaskType(value) ? value : 'agent_message'
}

export function toExecutionStep(raw: ExecutionStepCustomValue['step']): ExecutionStep {
  const content: StepContent = {
    type: raw.content.type,
    reasoning: raw.content.reasoning,
    planned_tools: raw.content.planned_tools,
    skill_id: raw.content.skill_id ? createSkillId(raw.content.skill_id) : undefined,
    skill_name: raw.content.skill_name,
    tool_name: raw.content.tool_name,
    tool_arguments: raw.content.tool_arguments,
    tool_result: raw.content.tool_result,
  }

  return {
    stepId: createExecutionId(raw.stepId),
    taskId: createTaskId(raw.taskId),
    status: raw.status,
    startedAt: raw.startedAt,
    completedAt: raw.completedAt,
    durationMs: raw.durationMs,
    errorMessage: raw.errorMessage,
    content,
  }
}

type RawExecutionStep = NonNullable<TaskCompletedCustomValue['executionSteps']>[number]

export function toExecutionStepFromCompleted(raw: RawExecutionStep): ExecutionStep {
  const content: StepContent = {
    type: raw.content.type as StepContent['type'],
  }

  return {
    stepId: createExecutionId(raw.stepId),
    taskId: createTaskId(raw.taskId),
    status: raw.status as ExecutionStep['status'],
    startedAt: raw.startedAt,
    completedAt: raw.completedAt,
    durationMs: raw.durationMs,
    content,
  }
}

export function toExecutionSteps(raw: RawExecutionStep[]): ExecutionStep[] {
  return raw.map(toExecutionStepFromCompleted)
}

export function toTaskFromRunStarted(raw: RunStartedCustomValue['task']): Task {
  const metadata: TaskMetadata = {
    ...raw.metadata,
    task_type: getTaskType(raw.metadata?.task_type),
    agent_name: raw.metadata?.agent_name || 'assistant',
    created_at: raw.metadata?.created_at || new Date().toISOString(),
  }

  return {
    id: raw.id,
    contextId: raw.contextId,
    status: raw.status,
    history: raw.history,
    artifacts: raw.artifacts ?? undefined,
    kind: raw.kind,
    metadata,
  } as Task
}

export function toTaskFromCompleted(raw: TaskCompletedCustomValue['task']): Task {
  const rawMetadata = raw.metadata || {}
  const metadata: TaskMetadata = {
    task_type: getTaskType(rawMetadata.task_type),
    agent_name: rawMetadata.agent_name || 'assistant',
    created_at: rawMetadata.created_at || new Date().toISOString(),
  }

  return {
    id: raw.id,
    contextId: raw.contextId,
    status: raw.status,
    history: raw.history,
    artifacts: raw.artifacts ?? undefined,
    kind: raw.kind,
    metadata,
  } as Task
}

export function toArtifactFromEvent(raw: unknown): Artifact | null {
  if (!raw || typeof raw !== 'object') {
    return null
  }

  if (!hasArtifactId(raw)) {
    return null
  }

  const artifact = raw as Record<string, unknown>
  const hasValidMetadata = artifact.metadata && typeof artifact.metadata === 'object'
  if (!hasValidMetadata) {
    return null
  }

  if (validateArtifact(raw as never)) {
    return raw as Artifact
  }

  return raw as Artifact
}
