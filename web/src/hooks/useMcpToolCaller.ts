import { useState, useCallback } from 'react'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js'
import { useContextStore } from '@/stores/context.store'
import { useAuthStore } from '@/stores/auth.store'
import { useUIStateStore } from '@/stores/ui-state.store'
import { logger } from '@/lib/logger'
import type { EphemeralArtifact } from '@/types/artifact'
import { createExecutionId, createTaskId, createArtifactId } from '@/types/core/brand'

interface ToolResponseWrapper {
  artifact_id: string
  mcp_execution_id: string
  artifact: Record<string, unknown>
  _metadata?: Record<string, unknown>
}

function isToolResponseWrapper(data: unknown): data is ToolResponseWrapper {
  return (
    typeof data === 'object' &&
    data !== null &&
    'artifact' in data &&
    'mcp_execution_id' in data &&
    typeof (data as ToolResponseWrapper).artifact === 'object'
  )
}

function constructEphemeralArtifact(
  structuredContent: unknown,
  toolName: string
): EphemeralArtifact {
  if (typeof structuredContent !== 'object' || structuredContent === null) {
    throw new Error('Invalid structured content: expected object')
  }

  if (!isToolResponseWrapper(structuredContent)) {
    throw new Error('structured_content is not a valid ToolResponse wrapper')
  }

  const executionId = structuredContent.mcp_execution_id
  const innerArtifact = structuredContent.artifact
  const artifactType = innerArtifact['x-artifact-type'] as string | undefined

  return {
    artifactId: executionId,
    name: toolName,
    description: `Result from ${toolName}`,
    parts: [
      {
        kind: 'data',
        data: innerArtifact
      }
    ],
    metadata: {
      ephemeral: true,
      artifact_type: artifactType || 'json',
      created_at: new Date().toISOString(),
      source: 'mcp_tool',
      tool_name: toolName,
      mcp_execution_id: executionId
    }
  }
}

export function useMcpToolCaller() {
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>(undefined)

  const currentContextId = useContextStore((state) => state.currentContextId)
  const createConversation = useContextStore((state) => state.createConversation)
  const getAuthHeader = useAuthStore((state) => state.getAuthHeader)

  const callTool = useCallback(
    async (
      serverEndpoint: string,
      toolName: string,
      toolArgs: Record<string, unknown>,
      serverName?: string
    ): Promise<void> => {
      logger.debug('Calling tool', {
        tool: toolName,
        paramCount: Object.keys(toolArgs).length
      }, 'useMcpToolCaller')

      const executionId = crypto.randomUUID()

      setLoading(true)
      setError(undefined)

      let contextId = currentContextId
      if (!contextId) {
        try {
          await createConversation('Tool Results')
          contextId = useContextStore.getState().currentContextId
          logger.debug('Created conversation', { contextId }, 'useMcpToolCaller')
        } catch (err) {
          logger.error('Failed to create conversation', err, 'useMcpToolCaller')
        }
      }

      const uiState = useUIStateStore.getState()
      uiState.addToolExecution(createTaskId(contextId || 'ephemeral'), {
        id: createExecutionId(executionId),
        toolName,
        serverName: serverName || 'Unknown',
        status: 'pending',
        timestamp: Date.now(),
        parameters: toolArgs
      })

      try {
        const authHeader = getAuthHeader()

        const headers: Record<string, string> = {
          'Accept': 'application/json, text/event-stream',
          'x-call-source': 'ephemeral'
        }

        if (authHeader) {
          headers['Authorization'] = authHeader
        }

        const traceId = crypto.randomUUID()
        headers['x-trace-id'] = traceId

        if (contextId) {
          headers['x-context-id'] = contextId
        }

        const apiBaseUrl = import.meta.env.VITE_API_BASE_HOST || window.location.origin
        const relativeEndpoint = serverEndpoint.replace(apiBaseUrl, '')

        const transport = new StreamableHTTPClientTransport(
          new URL(relativeEndpoint, window.location.origin),
          {
            requestInit: {
              headers,
            },
          }
        )

        const client = new Client(
          {
            name: 'systemprompt-web-client',
            version: '1.0.0',
          },
          {
            capabilities: {},
          }
        )

        await client.connect(transport)
        logger.debug('Connected to MCP server', { tool: toolName }, 'useMcpToolCaller')

        const result = await client.callTool({
          name: toolName,
          arguments: toolArgs,
        })

        logger.debug('Tool call result', {
          tool: toolName,
          isError: result.isError,
          contentItems: Array.isArray(result.content) ? result.content.length : 0,
          hasStructuredContent: !!result.structuredContent
        }, 'useMcpToolCaller')

        if (result.isError) {
          const content = result.content as Array<{ type: string; text?: string }>
          const errorMessage =
            content.find((c: { type: string; text?: string }) => c.type === 'text')?.text ||
            'Tool execution failed'
          throw new Error(errorMessage)
        }

        await client.close()

        if (result.structuredContent) {
          const ephemeralArtifact = constructEphemeralArtifact(
            result.structuredContent,
            toolName
          )

          useUIStateStore.getState().setEphemeralArtifact(ephemeralArtifact)
          useUIStateStore.getState().completeToolExecution(createExecutionId(executionId), createArtifactId(ephemeralArtifact.artifactId))

          logger.debug('Ephemeral artifact completed', { artifactId: ephemeralArtifact.artifactId }, 'useMcpToolCaller')
        }

        setLoading(false)
      } catch (err) {
        let errorMessage = 'Failed to call tool'
        if (err instanceof Error) {
          errorMessage = err.message
        } else if (typeof err === 'string') {
          errorMessage = err
        }

        logger.error('Error calling tool', err, 'useMcpToolCaller')
        setError(errorMessage)
        setLoading(false)

        useUIStateStore.getState().failToolExecution(createExecutionId(executionId), errorMessage)

        throw err
      }
    },
    [currentContextId, createConversation, getAuthHeader]
  )

  return {
    callTool,
    loading,
    error,
  }
}
