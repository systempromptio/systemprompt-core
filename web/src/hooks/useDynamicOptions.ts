import { useState, useEffect, useCallback, useRef } from 'react'
import type { DataSourceConfig } from '@/lib/schema/types'
import type { Artifact } from '@/types/artifact'
import { useToolsStore } from '@/stores/tools.store'
import { useMcpToolCaller } from './useMcpToolCaller'
import { useArtifactSubscription } from './useArtifactSubscription'
import { logger } from '@/lib/logger'

export interface DynamicOption {
  value: string
  label: string
  data?: unknown
}

export interface UseDynamicOptionsResult {
  options: DynamicOption[]
  loading: boolean
  error: string | null
  fetchFullObject: (uuid: string) => Promise<unknown>
}

export function useDynamicOptions(config: DataSourceConfig | undefined): UseDynamicOptionsResult {
  const [options, setOptions] = useState<DynamicOption[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const tools = useToolsStore((state) => state.tools)
  const { callTool } = useMcpToolCaller()

  const fullObjectResolverRef = useRef<{
    resolve: (data: unknown) => void
    reject: (error: Error) => void
  } | null>(null)

  const handleArtifact = useCallback((artifact: Artifact) => {
    try {
      const artifactData = extractDataFromArtifact(artifact)
      const items = extractItems(artifactData)
      const opts = items.map((item: unknown) => {
        if (typeof item !== 'object' || item === null) return { value: '', label: '' }
        const record = item as Record<string, unknown>
        const valueField = config?.value_field ?? ''
        const labelField = config?.label_field ?? ''
        return ({
          value: String(record[valueField] ?? ''),
          label: String(record[labelField] ?? '')
        })
      })
      setOptions(opts)
      setLoading(false)
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to extract options'
      setError(message)
      setLoading(false)
    }
  }, [config])

  const handleTimeout = useCallback(() => {
    setError('Tool execution timed out')
    setLoading(false)
  }, [])

  const { subscribe } = useArtifactSubscription({
    onArtifact: handleArtifact,
    onTimeout: handleTimeout,
    timeout: 30000
  })

  const handleFullObjectArtifact = useCallback((artifact: Artifact) => {
    if (fullObjectResolverRef.current) {
      const data = extractDataFromArtifact(artifact)
      if (data === undefined) {
        fullObjectResolverRef.current.reject(new Error('No data in artifact'))
      } else {
        fullObjectResolverRef.current.resolve(data)
      }
      fullObjectResolverRef.current = null
    }
  }, [])

  const handleFullObjectTimeout = useCallback(() => {
    if (fullObjectResolverRef.current) {
      fullObjectResolverRef.current.reject(new Error('Tool execution timed out'))
      fullObjectResolverRef.current = null
    }
  }, [])

  const { subscribe: subscribeFullObject } = useArtifactSubscription({
    onArtifact: handleFullObjectArtifact,
    onTimeout: handleFullObjectTimeout,
    timeout: 30000
  })

  useEffect(() => {
    if (!config) {
      setOptions([])
      return
    }

    const fetchOptions = async () => {
      setLoading(true)
      setError(null)

      try {
        const tool = tools.find((t) => t.name === config.tool)
        if (!tool) {
          throw new Error(`Tool '${config.tool}' not found`)
        }

        logger.debug('Fetching dynamic options', { tool: config.tool }, 'useDynamicOptions')

        const executionId = crypto.randomUUID()
        subscribe(executionId)

        await callTool(
          tool.serverEndpoint,
          tool.name,
          {
            action: config.action,
            ...config.filter
          }
        )
      } catch (err) {
        logger.error('Error fetching options', err, 'useDynamicOptions')
        const message = err instanceof Error ? err.message : 'Failed to fetch options'
        setError(message)
        setLoading(false)
      }
    }

    fetchOptions()
  }, [config, tools, callTool, subscribe])

  const fetchFullObject = useCallback(async (uuid: string): Promise<unknown> => {
    if (!config) {
      throw new Error('No data source configuration provided')
    }

    const tool = tools.find((t) => t.name === config.tool)
    if (!tool) {
      throw new Error(`Tool '${config.tool}' not found`)
    }

    logger.debug('Fetching full object', { tool: config.tool }, 'useDynamicOptions')

    const executionId = crypto.randomUUID()

    const artifactPromise = new Promise<unknown>((resolve, reject) => {
      fullObjectResolverRef.current = { resolve, reject }
      subscribeFullObject(executionId)
    })

    await callTool(
      tool.serverEndpoint,
      tool.name,
      { action: config.action, uuid }
    )

    return await artifactPromise
  }, [config, tools, callTool, subscribeFullObject])

  return { options, loading, error, fetchFullObject }
}

function hasProperty<K extends string>(
  obj: unknown,
  key: K
): obj is Record<K, unknown> {
  return typeof obj === 'object' && obj !== null && key in obj
}

function extractDataFromArtifact(artifact: unknown): unknown {
  if (!artifact || typeof artifact !== 'object') {
    return undefined
  }

  if (hasProperty(artifact, 'data')) {
    return artifact.data
  }

  if (hasProperty(artifact, 'parts') && Array.isArray(artifact.parts)) {
    for (const part of artifact.parts) {
      if (typeof part === 'object' && part !== null && hasProperty(part, 'kind')) {
        if (part.kind === 'data' && hasProperty(part, 'data')) {
          return part.data
        }
      }
    }
  }

  return artifact
}

function extractItems(data: unknown): unknown[] {
  if (Array.isArray(data)) {
    return data
  }

  if (typeof data === 'object' && data !== null) {
    if (hasProperty(data, 'items') && Array.isArray(data.items)) {
      return data.items
    }
    if (hasProperty(data, 'data') && Array.isArray(data.data)) {
      return data.data
    }
    if (hasProperty(data, 'results') && Array.isArray(data.results)) {
      return data.results
    }
  }

  return []
}

