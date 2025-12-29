export type { ArtifactType, ArtifactSource } from '@/types/artifact'

// Core artifact types - extensions can register additional types at runtime
const ARTIFACT_TYPES = [
  'text',
  'table',
  'chart',
  'form',
  'code',
  'tree',
  'json',
  'markdown',
  'dashboard',
  'presentation_card',
  'list',
  'copy_paste_text',
] as const

const ArtifactMetadataKey = {
  ARTIFACT_TYPE: 'artifact_type',
  CONTEXT_ID: 'context_id',
  TOOL_EXECUTION_ID: 'tool_execution_id',
  TOOL_NAME: 'tool_name',
  CREATED_AT: 'created_at',
  IS_INTERNAL: 'is_internal',
  RENDERING_HINTS: 'rendering_hints',
  SOURCE: 'source',
} as const

type ArtifactMetadataKey = (typeof ArtifactMetadataKey)[keyof typeof ArtifactMetadataKey]

const ARTIFACT_SOURCES = ['mcp_tool', 'agent', 'system', 'user'] as const

export { ARTIFACT_TYPES, ArtifactMetadataKey, ARTIFACT_SOURCES }
