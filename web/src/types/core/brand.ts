type Brand<T, B extends string> = T & { readonly __brand: B }

type UserId = Brand<string, 'UserId'>
type ContextId = Brand<string, 'ContextId'>
type TaskId = Brand<string, 'TaskId'>
type ArtifactId = Brand<string, 'ArtifactId'>
type AgentUrl = Brand<string, 'AgentUrl'>
type AuthToken = Brand<string, 'AuthToken'>
type SessionId = Brand<string, 'SessionId'>
type ConversationId = Brand<string, 'ConversationId'>
type MessageId = Brand<string, 'MessageId'>
type SkillId = Brand<string, 'SkillId'>
type ExecutionId = Brand<string, 'ExecutionId'>

function createUserId(id: string): UserId {
  return id as UserId
}

function createContextId(id: string): ContextId {
  return id as ContextId
}

function createTaskId(id: string): TaskId {
  return id as TaskId
}

function createArtifactId(id: string): ArtifactId {
  return id as ArtifactId
}

function createAgentUrl(url: string): AgentUrl {
  return url as AgentUrl
}

function createAuthToken(token: string): AuthToken {
  return token as AuthToken
}

function createSessionId(id: string): SessionId {
  return id as SessionId
}

function createConversationId(id: string): ConversationId {
  return id as ConversationId
}

function createMessageId(id: string): MessageId {
  return id as MessageId
}

function createSkillId(id: string): SkillId {
  return id as SkillId
}

function createExecutionId(id: string): ExecutionId {
  return id as ExecutionId
}

export type {
  Brand,
  UserId,
  ContextId,
  TaskId,
  ArtifactId,
  AgentUrl,
  AuthToken,
  SessionId,
  ConversationId,
  MessageId,
  SkillId,
  ExecutionId,
}

export {
  createUserId,
  createContextId,
  createTaskId,
  createArtifactId,
  createAgentUrl,
  createAuthToken,
  createSessionId,
  createConversationId,
  createMessageId,
  createSkillId,
  createExecutionId,
}
