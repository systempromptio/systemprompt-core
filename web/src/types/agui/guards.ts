import { AgUiEventType } from './event-types'
import type {
  AgUiEvent,
  CustomEvent,
  MessagesSnapshotEvent,
  RunErrorEvent,
  RunFinishedEvent,
  RunStartedEvent,
  StateDeltaEvent,
  StateSnapshotEvent,
  StepFinishedEvent,
  StepStartedEvent,
  TextMessageContentEvent,
  TextMessageEndEvent,
  TextMessageStartEvent,
  ToolCallArgsEvent,
  ToolCallEndEvent,
  ToolCallResultEvent,
  ToolCallStartEvent,
} from './events'

const AG_UI_EVENT_TYPES: Set<string> = new Set(Object.values(AgUiEventType))

export function isAgUiEvent(value: unknown): value is AgUiEvent {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return typeof candidate.type === 'string' && AG_UI_EVENT_TYPES.has(candidate.type)
}

export function isRunStartedEvent(event: AgUiEvent): event is RunStartedEvent {
  return event.type === AgUiEventType.RUN_STARTED
}

export function isRunFinishedEvent(event: AgUiEvent): event is RunFinishedEvent {
  return event.type === AgUiEventType.RUN_FINISHED
}

export function isRunErrorEvent(event: AgUiEvent): event is RunErrorEvent {
  return event.type === AgUiEventType.RUN_ERROR
}

export function isStepStartedEvent(event: AgUiEvent): event is StepStartedEvent {
  return event.type === AgUiEventType.STEP_STARTED
}

export function isStepFinishedEvent(event: AgUiEvent): event is StepFinishedEvent {
  return event.type === AgUiEventType.STEP_FINISHED
}

export function isTextMessageStartEvent(event: AgUiEvent): event is TextMessageStartEvent {
  return event.type === AgUiEventType.TEXT_MESSAGE_START
}

export function isTextMessageContentEvent(event: AgUiEvent): event is TextMessageContentEvent {
  return event.type === AgUiEventType.TEXT_MESSAGE_CONTENT
}

export function isTextMessageEndEvent(event: AgUiEvent): event is TextMessageEndEvent {
  return event.type === AgUiEventType.TEXT_MESSAGE_END
}

export function isToolCallStartEvent(event: AgUiEvent): event is ToolCallStartEvent {
  return event.type === AgUiEventType.TOOL_CALL_START
}

export function isToolCallArgsEvent(event: AgUiEvent): event is ToolCallArgsEvent {
  return event.type === AgUiEventType.TOOL_CALL_ARGS
}

export function isToolCallEndEvent(event: AgUiEvent): event is ToolCallEndEvent {
  return event.type === AgUiEventType.TOOL_CALL_END
}

export function isToolCallResultEvent(event: AgUiEvent): event is ToolCallResultEvent {
  return event.type === AgUiEventType.TOOL_CALL_RESULT
}

export function isStateSnapshotEvent(event: AgUiEvent): event is StateSnapshotEvent {
  return event.type === AgUiEventType.STATE_SNAPSHOT
}

export function isStateDeltaEvent(event: AgUiEvent): event is StateDeltaEvent {
  return event.type === AgUiEventType.STATE_DELTA
}

export function isMessagesSnapshotEvent(event: AgUiEvent): event is MessagesSnapshotEvent {
  return event.type === AgUiEventType.MESSAGES_SNAPSHOT
}

export function isCustomEvent(event: AgUiEvent): event is CustomEvent {
  return event.type === AgUiEventType.CUSTOM
}

export function isArtifactEvent(event: AgUiEvent): boolean {
  return isCustomEvent(event) && event.name === 'artifact'
}

export function isExecutionStepEvent(event: AgUiEvent): boolean {
  return isCustomEvent(event) && event.name === 'execution_step'
}

export function isSkillLoadedEvent(event: AgUiEvent): boolean {
  return isCustomEvent(event) && event.name === 'skill_loaded'
}

export function isRunStartedCustomEvent(event: AgUiEvent): boolean {
  return isCustomEvent(event) && event.name === 'run_started'
}

export function isTaskCompletedCustomEvent(event: AgUiEvent): boolean {
  return isCustomEvent(event) && event.name === 'task_completed'
}
