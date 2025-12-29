import { useCallback } from 'react'

import { logger } from '@/lib/logger'
import { useArtifactStore } from '@/stores/artifact.store'
import { useAuthStore } from '@/stores/auth.store'
import { useContextStore } from '@/stores/context.store'
import { useTaskStore } from '@/stores/task.store'
import { useUIStateStore } from '@/stores/ui-state.store'
import { useAgUiStore } from '@/stores/agui.store'
import { createTaskId, createContextId, createExecutionId } from '@/types/core/brand'
import {
  toExecutionStep,
  toExecutionSteps,
  toTaskFromRunStarted,
  toTaskFromCompleted,
  toArtifactFromEvent,
} from '@/lib/utils/agui-transformers'
import type {
  ArtifactCustomValue,
  CustomEvent,
  ExecutionStepCustomValue,
  RunErrorEvent,
  RunFinishedEvent,
  RunStartedEvent,
  RunStartedCustomValue,
  TaskCompletedCustomValue,
  SkillLoadedCustomValue,
  StateSnapshotEvent,
  StateDeltaEvent,
  StepFinishedEvent,
  StepStartedEvent,
  TextMessageContentEvent,
  TextMessageEndEvent,
  TextMessageStartEvent,
  ToolCallArgsEvent,
  ToolCallEndEvent,
  ToolCallResultEvent,
  ToolCallStartEvent,
} from '@/types/agui'
import {
  isAgUiEvent,
  isArtifactEvent,
  isCustomEvent,
  isExecutionStepEvent,
  isRunErrorEvent,
  isRunFinishedEvent,
  isRunStartedEvent,
  isRunStartedCustomEvent,
  isTaskCompletedCustomEvent,
  isSkillLoadedEvent,
  isStateSnapshotEvent,
  isStateDeltaEvent,
  isStepFinishedEvent,
  isStepStartedEvent,
  isTextMessageContentEvent,
  isTextMessageEndEvent,
  isTextMessageStartEvent,
  isToolCallArgsEvent,
  isToolCallEndEvent,
  isToolCallResultEvent,
  isToolCallStartEvent,
} from '@/types/agui'

export function useAgUiEventProcessor() {
  const processEvent = useCallback((data: string) => {
    let event: unknown
    try {
      event = JSON.parse(data)
    } catch {
      logger.error('Failed to parse AG-UI event', { data }, 'useAgUiEventProcessor')
      return
    }

    if (!isAgUiEvent(event)) {
      logger.warn('Invalid AG-UI event structure', { event }, 'useAgUiEventProcessor')
      return
    }

    logger.debug('Processing AG-UI event', { type: event.type }, 'useAgUiEventProcessor')

    if (isRunStartedEvent(event)) {
      handleRunStarted(event)
    } else if (isRunFinishedEvent(event)) {
      handleRunFinished(event)
    } else if (isRunErrorEvent(event)) {
      handleRunError(event)
    } else if (isStepStartedEvent(event)) {
      handleStepStarted(event)
    } else if (isStepFinishedEvent(event)) {
      handleStepFinished(event)
    } else if (isTextMessageStartEvent(event)) {
      handleTextMessageStart(event)
    } else if (isTextMessageContentEvent(event)) {
      handleTextMessageContent(event)
    } else if (isTextMessageEndEvent(event)) {
      handleTextMessageEnd(event)
    } else if (isToolCallStartEvent(event)) {
      handleToolCallStart(event)
    } else if (isToolCallArgsEvent(event)) {
      handleToolCallArgs(event)
    } else if (isToolCallEndEvent(event)) {
      handleToolCallEnd(event)
    } else if (isToolCallResultEvent(event)) {
      handleToolCallResult(event)
    } else if (isStateSnapshotEvent(event)) {
      handleStateSnapshot(event)
    } else if (isStateDeltaEvent(event)) {
      handleStateDelta(event)
    } else if (isCustomEvent(event)) {
      handleCustomEvent(event)
    }
  }, [])

  return { processEvent }
}

function handleRunStarted(event: RunStartedEvent): void {
  const { threadId, runId } = event
  useAgUiStore.getState().startRun(threadId, runId)
  useUIStateStore.getState().setStreaming(createTaskId(runId))
}

function handleRunFinished(event: RunFinishedEvent): void {
  const { threadId, runId } = event
  useAgUiStore.getState().endRun()
  useUIStateStore.getState().setStreaming(undefined)
  useUIStateStore.getState().clearStepsByTask(createTaskId(runId))
  const authHeader = useAuthStore.getState().getAuthHeader()
  useTaskStore.getState().fetchTasksByContext(createContextId(threadId), authHeader)
}

function handleRunError(event: RunErrorEvent): void {
  const { message, code } = event
  logger.error('Run error', { message, code }, 'AgUiEventProcessor')

  const aguiStore = useAgUiStore.getState()
  const threadId = aguiStore.currentThreadId
  const runId = aguiStore.currentRunId

  useAgUiStore.getState().endRun()
  useUIStateStore.getState().setStreaming(undefined)

  if (runId) {
    useUIStateStore.getState().clearStepsByTask(createTaskId(runId))
  }
  if (threadId) {
    const authHeader = useAuthStore.getState().getAuthHeader()
    useTaskStore.getState().fetchTasksByContext(createContextId(threadId), authHeader)
  }
}

function handleStepStarted(event: StepStartedEvent): void {
  const { stepName } = event
  const aguiStore = useAgUiStore.getState()
  const runId = aguiStore.currentRunId
  const threadId = aguiStore.currentThreadId

  if (runId && threadId) {
    useUIStateStore.getState().addStep(
      {
        stepId: createExecutionId(`step-${Date.now()}`),
        taskId: createTaskId(runId),
        status: 'in_progress',
        startedAt: event.timestamp,
        content: { type: stepName as 'understanding' | 'planning' | 'skill_usage' | 'tool_execution' | 'completion' },
      },
      createContextId(threadId)
    )
  }
}

function handleStepFinished(event: StepFinishedEvent): void {
  const { stepName } = event
  logger.debug('Step finished', { stepName }, 'AgUiEventProcessor')
}

function handleTextMessageStart(event: TextMessageStartEvent): void {
  const { messageId, role } = event
  useAgUiStore.getState().startMessage(messageId, role)
}

function handleTextMessageContent(event: TextMessageContentEvent): void {
  const { messageId, delta } = event
  useAgUiStore.getState().appendMessageContent(messageId, delta)
}

function handleTextMessageEnd(event: TextMessageEndEvent): void {
  const { messageId } = event
  const message = useAgUiStore.getState().endMessage(messageId)
  if (message) {
    logger.debug('Message completed', { messageId, length: message.content.length }, 'AgUiEventProcessor')
  }
}

function handleToolCallStart(event: ToolCallStartEvent): void {
  const { toolCallId, toolCallName } = event
  useAgUiStore.getState().startToolCall(toolCallId, toolCallName)

  const aguiStore = useAgUiStore.getState()
  const taskId = aguiStore.currentRunId

  if (taskId) {
    useUIStateStore.getState().addToolExecution(createTaskId(taskId), {
      id: createExecutionId(toolCallId),
      toolName: toolCallName,
      serverName: '',
      status: 'executing',
      timestamp: Date.now(),
    })
  }
}

function handleToolCallArgs(event: ToolCallArgsEvent): void {
  const { toolCallId, delta } = event
  useAgUiStore.getState().appendToolArgs(toolCallId, delta)
}

function handleToolCallEnd(event: ToolCallEndEvent): void {
  const { toolCallId } = event
  const toolCall = useAgUiStore.getState().endToolCall(toolCallId)
  if (toolCall) {
    logger.debug('Tool call completed', { toolCallId, name: toolCall.name }, 'AgUiEventProcessor')
  }
}

function handleToolCallResult(event: ToolCallResultEvent): void {
  const { toolCallId } = event
  useUIStateStore.getState().completeToolExecution(createExecutionId(toolCallId))
}

function handleStateSnapshot(event: StateSnapshotEvent): void {
  const { snapshot } = event
  useAgUiStore.getState().setSnapshot(snapshot as Record<string, unknown>)

  if (snapshot && typeof snapshot === 'object' && 'contexts' in snapshot) {
    const contexts = (snapshot as { contexts: unknown }).contexts
    if (Array.isArray(contexts)) {
      useContextStore.getState().handleSnapshot(contexts)
    }
  }
}

function handleStateDelta(event: StateDeltaEvent): void {
  const { delta } = event
  useAgUiStore.getState().applyDelta(delta)
}

function handleCustomEvent(event: CustomEvent): void {
  const { name, value } = event

  if (isArtifactEvent(event)) {
    handleArtifactCustomEvent(value as ArtifactCustomValue)
  } else if (isExecutionStepEvent(event)) {
    handleExecutionStepCustomEvent(value as ExecutionStepCustomValue)
  } else if (isSkillLoadedEvent(event)) {
    handleSkillLoadedCustomEvent(value as SkillLoadedCustomValue)
  } else if (isRunStartedCustomEvent(event)) {
    handleRunStartedCustomEvent(value as RunStartedCustomValue)
  } else if (isTaskCompletedCustomEvent(event)) {
    handleTaskCompletedCustomEvent(value as TaskCompletedCustomValue)
  } else {
    logger.debug('Unknown custom event', { name }, 'AgUiEventProcessor')
  }
}

function handleExecutionStepCustomEvent(value: ExecutionStepCustomValue): void {
  const { step, contextId } = value
  const transformedStep = toExecutionStep(step)
  useUIStateStore.getState().addStep(transformedStep, createContextId(contextId))
}

function handleArtifactCustomEvent(value: ArtifactCustomValue): void {
  const { artifact, taskId, contextId } = value
  const transformedArtifact = toArtifactFromEvent(artifact)
  if (transformedArtifact) {
    useArtifactStore.getState().addArtifact(transformedArtifact, createTaskId(taskId), createContextId(contextId))
    useUIStateStore.getState().completeToolExecutionByArtifact({
      artifactId: transformedArtifact.artifactId,
      metadata: transformedArtifact.metadata,
    })
  }
}

function handleSkillLoadedCustomEvent(value: SkillLoadedCustomValue): void {
  const { skillId, skillName, description, taskId } = value
  logger.debug('Skill loaded', { skillId, skillName, description, taskId }, 'AgUiEventProcessor')
}

function handleRunStartedCustomEvent(value: RunStartedCustomValue): void {
  const { task } = value
  if (task && task.id && task.contextId) {
    const transformedTask = toTaskFromRunStarted(task)
    useTaskStore.getState().updateTask(transformedTask)
    logger.debug('Task added from run_started', { taskId: task.id, contextId: task.contextId }, 'AgUiEventProcessor')
  } else {
    logger.warn('Invalid task in run_started event', { task }, 'AgUiEventProcessor')
  }
}

function handleTaskCompletedCustomEvent(value: TaskCompletedCustomValue): void {
  const { task, executionSteps } = value
  if (task && task.id && task.contextId) {
    const transformedTask = toTaskFromCompleted(task)
    useTaskStore.getState().updateTask(transformedTask)

    if (executionSteps && executionSteps.length > 0) {
      const transformedSteps = toExecutionSteps(executionSteps)
      useUIStateStore.getState().addSteps(transformedSteps, createContextId(task.contextId))
    }

    logger.debug('Task completed from custom event', { taskId: task.id }, 'AgUiEventProcessor')
  } else {
    logger.warn('Invalid task in task_completed event', { task }, 'AgUiEventProcessor')
  }
}
