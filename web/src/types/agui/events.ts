import type {
  CustomPayload,
  MessagesSnapshotPayload,
  RunErrorPayload,
  RunFinishedPayload,
  RunStartedPayload,
  StateDeltaPayload,
  StateSnapshotPayload,
  StepFinishedPayload,
  StepStartedPayload,
  TextMessageContentPayload,
  TextMessageEndPayload,
  TextMessageStartPayload,
  ToolCallArgsPayload,
  ToolCallEndPayload,
  ToolCallResultPayload,
  ToolCallStartPayload,
} from './payloads'

interface BaseEvent {
  timestamp: string
}

export interface RunStartedEvent extends BaseEvent, RunStartedPayload {
  type: 'RUN_STARTED'
}

export interface RunFinishedEvent extends BaseEvent, RunFinishedPayload {
  type: 'RUN_FINISHED'
}

export interface RunErrorEvent extends BaseEvent, RunErrorPayload {
  type: 'RUN_ERROR'
}

export interface StepStartedEvent extends BaseEvent, StepStartedPayload {
  type: 'STEP_STARTED'
}

export interface StepFinishedEvent extends BaseEvent, StepFinishedPayload {
  type: 'STEP_FINISHED'
}

export interface TextMessageStartEvent extends BaseEvent, TextMessageStartPayload {
  type: 'TEXT_MESSAGE_START'
}

export interface TextMessageContentEvent extends BaseEvent, TextMessageContentPayload {
  type: 'TEXT_MESSAGE_CONTENT'
}

export interface TextMessageEndEvent extends BaseEvent, TextMessageEndPayload {
  type: 'TEXT_MESSAGE_END'
}

export interface ToolCallStartEvent extends BaseEvent, ToolCallStartPayload {
  type: 'TOOL_CALL_START'
}

export interface ToolCallArgsEvent extends BaseEvent, ToolCallArgsPayload {
  type: 'TOOL_CALL_ARGS'
}

export interface ToolCallEndEvent extends BaseEvent, ToolCallEndPayload {
  type: 'TOOL_CALL_END'
}

export interface ToolCallResultEvent extends BaseEvent, ToolCallResultPayload {
  type: 'TOOL_CALL_RESULT'
}

export interface StateSnapshotEvent extends BaseEvent, StateSnapshotPayload {
  type: 'STATE_SNAPSHOT'
}

export interface StateDeltaEvent extends BaseEvent, StateDeltaPayload {
  type: 'STATE_DELTA'
}

export interface MessagesSnapshotEvent extends BaseEvent, MessagesSnapshotPayload {
  type: 'MESSAGES_SNAPSHOT'
}

export interface CustomEvent extends BaseEvent, CustomPayload {
  type: 'CUSTOM'
}

export type AgUiEvent =
  | RunStartedEvent
  | RunFinishedEvent
  | RunErrorEvent
  | StepStartedEvent
  | StepFinishedEvent
  | TextMessageStartEvent
  | TextMessageContentEvent
  | TextMessageEndEvent
  | ToolCallStartEvent
  | ToolCallArgsEvent
  | ToolCallEndEvent
  | ToolCallResultEvent
  | StateSnapshotEvent
  | StateDeltaEvent
  | MessagesSnapshotEvent
  | CustomEvent
