import { useCallback, useEffect, useRef, useState } from 'react'

import { logger } from '@/lib/logger'
import { useAuthStore } from '@/stores/auth.store'
import { useAgUiEventProcessor } from './useAgUiEventProcessor'

interface UseAgUiConnectionOptions {
  url: string
  onConnected?: () => void
  onDisconnected?: () => void
  onError?: (error: Error) => void
}

interface UseAgUiConnectionReturn {
  isConnected: boolean
  reconnect: () => void
  disconnect: () => void
}

const MAX_RECONNECT_ATTEMPTS = 5
const BASE_RECONNECT_DELAY = 1000

export function useAgUiConnection(options: UseAgUiConnectionOptions): UseAgUiConnectionReturn {
  const { url, onConnected, onDisconnected, onError } = options
  const { processEvent } = useAgUiEventProcessor()
  const [isConnected, setIsConnected] = useState(false)
  const readerRef = useRef<ReadableStreamDefaultReader<Uint8Array> | null>(null)
  const abortControllerRef = useRef<AbortController | null>(null)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null)

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
      abortControllerRef.current = null
    }
    if (readerRef.current) {
      readerRef.current.cancel().catch(() => {})
      readerRef.current = null
    }
    setIsConnected(false)
  }, [])

  const connect = useCallback(async () => {
    disconnect()

    const token = useAuthStore.getState().accessToken
    if (!token) {
      logger.warn('No auth token available', {}, 'useAgUiConnection')
      return
    }

    const controller = new AbortController()
    abortControllerRef.current = controller

    try {
      const response = await fetch(url, {
        method: 'GET',
        headers: {
          Accept: 'text/event-stream',
          Authorization: `Bearer ${token}`,
          'Cache-Control': 'no-cache',
        },
        signal: controller.signal,
      })

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`)
      }

      if (!response.body) {
        throw new Error('No response body')
      }

      setIsConnected(true)
      reconnectAttemptRef.current = 0
      onConnected?.()

      const reader = response.body.getReader()
      readerRef.current = reader
      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() ?? ''

        for (const line of lines) {
          if (line.startsWith('data:')) {
            const data = line.slice(5).trim()
            if (data) {
              processEvent(data)
            }
          }
        }
      }
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        return
      }

      logger.error('Connection error', error, 'useAgUiConnection')
      onError?.(error instanceof Error ? error : new Error(String(error)))
      setIsConnected(false)
      onDisconnected?.()

      if (reconnectAttemptRef.current < MAX_RECONNECT_ATTEMPTS) {
        const delay = BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttemptRef.current)
        reconnectAttemptRef.current += 1
        reconnectTimeoutRef.current = setTimeout(() => {
          connect()
        }, delay)
      }
    }
  }, [url, disconnect, processEvent, onConnected, onDisconnected, onError])

  const reconnect = useCallback(() => {
    reconnectAttemptRef.current = 0
    connect()
  }, [connect])

  useEffect(() => {
    connect()
    return () => {
      disconnect()
    }
  }, [connect, disconnect])

  return { isConnected, reconnect, disconnect }
}
