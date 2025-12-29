import { useState, useCallback } from 'react'
import type { McpTool } from '@/stores/tools.store'
import type { FormValues } from '@/lib/schema/types'
import { canAutoSubmit, extractDefaults } from '@/lib/schema/defaults'
import { extractToolInputSchema } from '@/lib/schema/validation'
import { useMcpToolCaller } from './useMcpToolCaller'
import { logger } from '@/lib/logger'

export function useToolParameters() {
  const [showModal, setShowModal] = useState(false)
  const [selectedTool, setSelectedTool] = useState<McpTool | null>(null)
  const { callTool } = useMcpToolCaller()

  const executeTool = useCallback(
    async (tool: McpTool) => {
      logger.debug('Executing tool', { tool: tool.name }, 'useToolParameters')

      const schema = extractToolInputSchema(tool.inputSchema, tool.name)

      if (canAutoSubmit(schema)) {
        logger.debug('Auto-submitting with defaults', { tool: tool.name }, 'useToolParameters')
        const defaults = extractDefaults(schema)
        await callTool(
          tool.serverEndpoint,
          tool.name,
          defaults,
          tool.serverName
        )
        return
      }

      setSelectedTool(tool)
      setShowModal(true)
    },
    [callTool]
  )

  const submitParameters = useCallback(
    async (tool: McpTool, parameters: FormValues) => {
      logger.debug('Submitting parameters', { tool: tool.name }, 'useToolParameters')
      await callTool(
        tool.serverEndpoint,
        tool.name,
        parameters,
        tool.serverName
      )
    },
    [callTool]
  )

  const closeModal = useCallback(() => {
    setShowModal(false)
    setSelectedTool(null)
  }, [])

  return {
    executeTool,
    submitParameters,
    closeModal,
    showModal,
    selectedTool,
  }
}
