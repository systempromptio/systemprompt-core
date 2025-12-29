import { useState } from 'react'
import { useA2AClientInitialization } from './useA2AClientInitialization'
import { useA2AClientReady } from './useA2AClientReady'
import { useA2AMessageOperations } from './useA2AMessageOperations'

export function useA2AClient() {
  const [error, setError] = useState<Error | undefined>(undefined)

  const {
    client,
    loading,
    error: initError,
    retrying,
    retryConnection
  } = useA2AClientInitialization()

  const { ensureClientReady } = useA2AClientReady(client, retryConnection)

  const {
    sendMessage,
    streamMessage,
    getTask,
    cancelTask
  } = useA2AMessageOperations(client, ensureClientReady, setError)

  return {
    client,
    loading,
    error: error || initError,
    retrying,
    retryConnection,
    sendMessage,
    streamMessage,
    getTask,
    cancelTask,
  }
}