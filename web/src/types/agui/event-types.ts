export const AgUiEventType = {
  RUN_STARTED: 'RUN_STARTED',
  RUN_FINISHED: 'RUN_FINISHED',
  RUN_ERROR: 'RUN_ERROR',
  STEP_STARTED: 'STEP_STARTED',
  STEP_FINISHED: 'STEP_FINISHED',
  TEXT_MESSAGE_START: 'TEXT_MESSAGE_START',
  TEXT_MESSAGE_CONTENT: 'TEXT_MESSAGE_CONTENT',
  TEXT_MESSAGE_END: 'TEXT_MESSAGE_END',
  TOOL_CALL_START: 'TOOL_CALL_START',
  TOOL_CALL_ARGS: 'TOOL_CALL_ARGS',
  TOOL_CALL_END: 'TOOL_CALL_END',
  TOOL_CALL_RESULT: 'TOOL_CALL_RESULT',
  STATE_SNAPSHOT: 'STATE_SNAPSHOT',
  STATE_DELTA: 'STATE_DELTA',
  MESSAGES_SNAPSHOT: 'MESSAGES_SNAPSHOT',
  CUSTOM: 'CUSTOM',
} as const

export type AgUiEventTypeValue = (typeof AgUiEventType)[keyof typeof AgUiEventType]

export const MessageRole = {
  USER: 'user',
  ASSISTANT: 'assistant',
  SYSTEM: 'system',
  TOOL: 'tool',
} as const

export type MessageRoleValue = (typeof MessageRole)[keyof typeof MessageRole]
