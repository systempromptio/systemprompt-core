import { A2AClient, createAuthenticatingFetchWithRetry, type AuthenticationHandler } from '@a2a-js/sdk/client'
import type {
  AgentCard,
  Part,
  SendMessageResponse,
  GetTaskResponse,
  CancelTaskResponse,
  JSONRPCErrorResponse,
  SendMessageSuccessResponse,
  GetTaskSuccessResponse,
  CancelTaskSuccessResponse,
  Task,
  Message,
  TaskStatusUpdateEvent,
  TaskArtifactUpdateEvent
} from '@a2a-js/sdk'

type A2AStreamEventData = Message | Task | TaskStatusUpdateEvent | TaskArtifactUpdateEvent

export class A2AService {
  private client: A2AClient | undefined = undefined
  private agentUrl: string
  private agentCard: AgentCard | undefined = undefined
  private _authToken: string | undefined = undefined
  private isRefreshingToken: boolean = false
  private refreshPromise: Promise<string | undefined> | undefined = undefined

  constructor(agentUrl: string, authToken?: string) {
    this.agentUrl = agentUrl
    this._authToken = authToken
  }

  setAuthToken(token: string | undefined) {
    this._authToken = token
  }

  getAuthToken(): string | undefined {
    return this._authToken
  }

  resetClient(): void {
    this.client = undefined
    this.agentCard = undefined
    this.isRefreshingToken = false
    this.refreshPromise = undefined
  }

  private async refreshToken(): Promise<string | undefined> {
    if (this.isRefreshingToken && this.refreshPromise) {
      return this.refreshPromise
    }

    this.isRefreshingToken = true
    this.refreshPromise = this.performTokenRefresh()

    try {
      const newToken = await this.refreshPromise
      return newToken
    } finally {
      this.isRefreshingToken = false
      this.refreshPromise = undefined
    }
  }

  private async performTokenRefresh(): Promise<string | undefined> {
    try {
      const { useAuthStore } = await import('@/stores/auth.store')
      const { authService } = await import('@/services/auth.service')

      const userType = useAuthStore.getState().userType

      if (userType === 'anon') {

        const { token, error } = await authService.generateAnonymousToken()

        if (error || !token) {
          useAuthStore.getState().clearAuth()
          return undefined
        }

        useAuthStore.getState().setAnonymousAuth(
          token.access_token,
          token.user_id,
          token.session_id,
          token.expires_in
        )

        return `Bearer ${token.access_token}`
      } else {
        useAuthStore.getState().clearAuth()
        return undefined
      }
    } catch {
      return undefined
    }
  }

  async initialize(existingCard?: AgentCard): Promise<AgentCard> {
    const authHandler: AuthenticationHandler = {
      headers: async () => {
        const headers: Record<string, string> = {}
        if (this._authToken) {
          headers['Authorization'] = this._authToken
        }
        headers['x-trace-id'] = crypto.randomUUID()
        return headers
      },
      shouldRetryWithHeaders: async (_req, res) => {
        // 401 = invalid/expired token, try refresh
        // 403 = valid token but insufficient permissions, don't refresh
        if (res.status === 401) {
          const newToken = await this.refreshToken()
          if (newToken) {
            this._authToken = newToken
            return { 'Authorization': newToken }
          }
        }
        return undefined
      }
    }

    const authFetch = createAuthenticatingFetchWithRetry(fetch, authHandler)

    if (existingCard) {
      this.client = new A2AClient(existingCard, { fetchImpl: authFetch })
      this.agentCard = existingCard
      return this.agentCard
    }

    try {
      const cardUrl = `${this.agentUrl}/.well-known/agent-card.json`
      this.client = await A2AClient.fromCardUrl(cardUrl, { fetchImpl: authFetch })
      this.agentCard = await this.client.getAgentCard()
      return this.agentCard
    } catch (error) {
      throw new Error(`Failed to fetch agent card from ${this.agentUrl}/.well-known/agent-card.json: ${error}`)
    }
  }

  async sendMessage(text: string, files: File[] | undefined, contextId: string): Promise<Task | Message> {
    if (!this.client) {
      throw new Error('Client not initialized. Please wait for initialization or refresh the page.')
    }

    const parts: Part[] = [{ kind: 'text' as const, text }]

    if (files?.length) {
      for (const file of files) {
        const bytes = await this.fileToBase64(file)
        parts.push({
          kind: 'file' as const,
          file: {
            name: file.name,
            mimeType: file.type,
            bytes,
          },
        })
      }
    }

    const response = await this.client.sendMessage({
      message: {
        kind: 'message' as const,
        role: 'user' as const,
        parts,
        messageId: crypto.randomUUID() as `${string}-${string}-${string}-${string}-${string}`,
        contextId: contextId as `${string}-${string}-${string}-${string}-${string}`,
      },
    })

    if (this.isErrorResponse(response)) {
      throw new Error(`A2A Error: ${response.error.message}`)
    }

    const result = (response as SendMessageSuccessResponse).result
    if (!result) {
      throw new Error('No result returned from sendMessage')
    }
    return result
  }

  async* streamMessage(text: string, contextId: string, clientMessageId?: string): AsyncGenerator<A2AStreamEventData> {
    if (!this.client) {
      throw new Error('Client not initialized. Please wait for initialization or refresh the page.')
    }

    const callbackUrl = `${window.location.origin}/api/v1/core/contexts/${contextId}/notifications`

    const stream = this.client.sendMessageStream({
      message: {
        kind: 'message' as const,
        role: 'user' as const,
        parts: [{ kind: 'text' as const, text }],
        messageId: crypto.randomUUID() as `${string}-${string}-${string}-${string}-${string}`,
        contextId: contextId as `${string}-${string}-${string}-${string}-${string}`,
        metadata: clientMessageId ? { clientMessageId } : undefined,
      },
      configuration: {
        pushNotificationConfig: {
          url: callbackUrl,
          token: this._authToken || undefined,
        }
      },
    })

    for await (const event of stream) {
      yield event
    }
  }

  async getTask(taskId: string): Promise<Task> {
    if (!this.client) {
      throw new Error('Client not initialized. Please wait for initialization or refresh the page.')
    }
    const response = await this.client.getTask({ id: taskId })

    if (this.isErrorResponse(response)) {
      throw new Error(`A2A Error: ${response.error.message}`)
    }

    const result = (response as GetTaskSuccessResponse).result
    if (!result) {
      throw new Error(`Task not found: ${taskId}`)
    }
    return result
  }

  async cancelTask(taskId: string): Promise<Task> {
    if (!this.client) {
      throw new Error('Client not initialized. Please wait for initialization or refresh the page.')
    }
    const response = await this.client.cancelTask({ id: taskId })

    if (this.isErrorResponse(response)) {
      throw new Error(`A2A Error: ${response.error.message}`)
    }

    const result = (response as CancelTaskSuccessResponse).result
    if (!result) {
      throw new Error(`Failed to cancel task: ${taskId}`)
    }
    return result
  }

  getAgentCard(): AgentCard {
    if (!this.agentCard) {
      throw new Error('Agent card not initialized. Call initialize() first.')
    }
    return this.agentCard
  }

  hasAgentCard(): boolean {
    return this.agentCard !== undefined
  }

  private isErrorResponse(response: SendMessageResponse | GetTaskResponse | CancelTaskResponse): response is JSONRPCErrorResponse {
    return 'error' in response
  }

  private async fileToBase64(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.readAsDataURL(file)
      reader.onload = () => {
        const base64 = reader.result as string
        const base64Data = base64.split(',')[1]
        resolve(base64Data)
      }
      reader.onerror = reject
    })
  }
}

const clients = new Map<string, A2AService>()

export function getA2AClient(
  agentUrl: string,
  authToken?: string
): A2AService {
  const cacheKey = agentUrl

  if (!clients.has(cacheKey)) {
    clients.set(cacheKey, new A2AService(agentUrl, authToken))
  } else {
    const client = clients.get(cacheKey)!
    if (authToken !== client.getAuthToken()) {
      client.setAuthToken(authToken)
    }
  }
  return clients.get(cacheKey)!
}

export function clearA2AClient(agentUrl: string): void {
  clients.delete(agentUrl)
}