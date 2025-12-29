export type {
  AgentCard,
  Message,
  Task,
  TaskState,
  TaskStatus,
  Artifact,
  Part,
  TextPart,
  FilePart,
  DataPart,
  AgentSkill,
  AgentCapabilities,
} from '@a2a-js/sdk'

import type { Part, Artifact, Task, Message, TaskStatus } from '@a2a-js/sdk'

export interface ChatMessage {
  id: string
  timestamp: number
  content: string
  role: 'user' | 'assistant'
  parts?: Part[]
  isStreaming?: boolean
  artifacts?: Artifact[]
  task?: Task
  error?: string
}

export interface StreamEvent {
  kind: 'message' | 'artifact-update' | 'status-update' | 'task'
  data: Message | Artifact | TaskStatus | Task
}

export interface AgentEndpoint {
  url: string
  name: string
  description?: string
}