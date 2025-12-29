import type { Task } from '@/types/task'
import type { Task as A2ATask, TaskStatus, Artifact as A2AArtifact, Message } from '@a2a-js/sdk'
import type { Artifact, EphemeralArtifact } from '@/types/artifact'
import { validateArtifact } from '@/types/artifact'

export function isPersistedArtifact(artifact: unknown): artifact is Artifact {
  if (!artifact || typeof artifact !== 'object') return false

  const record = artifact as Record<string, unknown>
  return (
    typeof record.id === 'string' &&
    typeof record.name === 'string' &&
    typeof record.description === 'string'
  )
}

export function isEphemeralArtifact(artifact: unknown): artifact is EphemeralArtifact {
  if (!artifact || typeof artifact !== 'object') return false

  const record = artifact as Record<string, unknown>
  const metadata = record.metadata as Record<string, unknown> | undefined
  return metadata?.ephemeral === true
}

export function isValidTaskState(value: unknown): value is string {
  const validStates = [
    'submitted',
    'working',
    'input-required',
    'completed',
    'failed',
    'rejected',
    'canceled',
    'auth-required',
  ]

  return typeof value === 'string' && validStates.includes(value)
}

export function isTerminalTask(task: Task | undefined): task is Task {
  if (!task?.status) return false

  const terminalStates = ['completed', 'failed', 'rejected', 'canceled']
  return terminalStates.includes(task.status.state)
}

export function isRunningTask(task: Task | undefined): task is Task {
  if (!task?.status) return false

  const runningStates = ['submitted', 'working', 'input-required']
  return runningStates.includes(task.status.state)
}

export function isFailedTask(task: Task | undefined): task is Task {
  if (!task?.status) return false

  return ['failed', 'rejected'].includes(task.status.state)
}

export function isInputRequiredTask(task: Task | undefined): task is Task {
  return task?.status?.state === 'input-required'
}

export function isAuthRequiredTask(task: Task | undefined): task is Task {
  return task?.status?.state === 'auth-required'
}

export function hasTaskMetadata(task: Task | undefined): boolean {
  return task?.metadata !== undefined
}

export function isTask(value: unknown): value is Task {
  if (!value || typeof value !== 'object') return false

  const record = value as Record<string, unknown>
  return (
    typeof record.id === 'string' &&
    record.status !== undefined &&
    typeof record.status === 'object'
  )
}

export function isTaskStatus(value: unknown): value is TaskStatus {
  if (!value || typeof value !== 'object') return false

  const record = value as Record<string, unknown>
  return (
    typeof record.state === 'string' &&
    typeof record.timestamp === 'string'
  )
}

export function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

export function isUUID(value: unknown): value is string {
  if (typeof value !== 'string') return false

  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
  return uuidRegex.test(value)
}

export function isValidURL(value: unknown): value is string {
  if (typeof value !== 'string') return false

  try {
    new URL(value)
    return true
  } catch {
    return false
  }
}

export function isValidEmail(value: unknown): value is string {
  if (typeof value !== 'string') return false

  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
  return emailRegex.test(value)
}

export function isNullish(value: unknown): value is undefined {
  return value === undefined
}

export function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

export function isError(error: unknown): error is Error {
  return error instanceof Error
}

export function hasTaskInData(data: unknown): data is { task: Task } {
  if (!isPlainObject(data)) return false
  if (!('task' in data)) return false
  return isTask(data.task)
}

export function hasArtifactInData(data: unknown): data is { artifact: A2AArtifact } {
  if (!isPlainObject(data)) return false
  if (!('artifact' in data)) return false
  return validateArtifact(data.artifact as A2AArtifact)
}

export function hasMessageInData(data: unknown): data is { message: Message } {
  if (!isPlainObject(data)) return false
  if (!('message' in data)) return false
  const message = data.message
  return isPlainObject(message) && 'role' in message && Array.isArray((message as Record<string, unknown>).parts)
}

export function hasContextInData(data: unknown): data is { context_id: string } {
  if (!isPlainObject(data)) return false
  return 'context_id' in data && typeof data.context_id === 'string'
}

interface StatusMessage {
  messageId: string
  role?: string
  content?: string
}

export function isStatusMessage(value: unknown): value is StatusMessage {
  if (!isPlainObject(value)) return false
  return 'messageId' in value && typeof value.messageId === 'string'
}

export function getStatusMessageId(task: Task): string | undefined {
  if (!task.status?.message) return undefined
  if (isStatusMessage(task.status.message)) {
    return task.status.message.messageId
  }
  return undefined
}

export function extractFirstMessageText(task: A2ATask): string | undefined {
  const history = task.history
  if (!history || history.length === 0) return undefined

  const firstMessage = history[0]
  if (!firstMessage.parts || firstMessage.parts.length === 0) return undefined

  const firstPart = firstMessage.parts[0]
  if (!('text' in firstPart) || typeof firstPart.text !== 'string') return undefined

  return firstPart.text.substring(0, 100)
}
