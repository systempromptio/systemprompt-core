import { logger } from '@/lib/logger'
import type { AsyncResult } from '@/types/core'
import {
  Ok,
  Err,
  createNetworkError,
  createUnauthorizedError,
  createForbiddenError,
  createRateLimitError,
  createServerError,
  createUnknownError,
} from '@/types/core'
import type { ApiError } from '@/types/core'

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH'
  headers?: Record<string, string>
  body?: unknown
  authToken?: string | undefined
  _retryCount?: number
}

type RefreshState =
  | { status: 'idle' }
  | { status: 'refreshing'; promise: Promise<string | undefined> }

class ApiClient {
  private baseUrl: string
  private refreshState: RefreshState = { status: 'idle' }

  constructor(baseUrl?: string) {
    if (baseUrl) {
      this.baseUrl = baseUrl
    } else {
      const host = import.meta.env.VITE_API_BASE_HOST
      const path = import.meta.env.VITE_API_BASE_PATH

      if (host && path) {
        this.baseUrl = `${host}${path}`
      } else if (host) {
        this.baseUrl = `${host}/api/v1/core`
      } else if (path) {
        this.baseUrl = path
      } else {
        this.baseUrl = import.meta.env.VITE_API_BASE_URL || '/api/v1/core'
      }
    }
  }

  private async refreshToken(): Promise<string | undefined> {
    if (this.refreshState.status === 'refreshing') {
      return this.refreshState.promise
    }

    const promise = this.performTokenRefresh()
    this.refreshState = { status: 'refreshing', promise }

    try {
      const result = await promise
      return result
    } finally {
      this.refreshState = { status: 'idle' }
    }
  }

  private async performTokenRefresh(): Promise<string | undefined> {
    try {
      const { useAuthStore } = await import('@/stores/auth.store')
      await import('./auth.service')

      const userType = useAuthStore.getState().userType

      if (userType === 'anon') {
        logger.debug('Refreshing anonymous token due to 401', undefined, 'api-client')
        return await this.refreshAnonymousToken()
      }

      logger.debug('Authenticated user token expired, clearing auth', undefined, 'api-client')
      useAuthStore.getState().clearAuth()
      return undefined
    } catch (error) {
      logger.error('Error during token refresh', error, 'api-client')
      return undefined
    }
  }

  private async refreshAnonymousToken(): Promise<string | undefined> {
    try {
      const { useAuthStore } = await import('@/stores/auth.store')
      const { authService } = await import('./auth.service')

      const { token, error } = await authService.generateAnonymousToken()

      if (error || !token) {
        logger.error('Failed to refresh token', error, 'api-client')
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
    } catch (error) {
      logger.error('Error generating anonymous token', error, 'api-client')
      return undefined
    }
  }

  private async extractErrorText(response: Response): Promise<string> {
    const contentType = response.headers.get('content-type')
    const isJson = contentType?.includes('application/json')

    try {
      if (isJson) {
        const data = await response.json()
        return data.message || ''
      }
      return await response.text()
    } catch {
      return ''
    }
  }

  private async handle401Error<T>(
    response: Response,
    endpoint: string,
    options: RequestOptions,
  ): AsyncResult<T, ApiError> {
    const responseText = await this.extractErrorText(response)

    // 401 = invalid/expired token, always try refresh (only once)
    if ((options._retryCount || 0) === 0) {
      logger.debug('401 received, attempting token refresh', undefined, 'api-client')
      const newToken = await this.refreshToken()

      if (newToken) {
        logger.debug('Token refreshed, retrying request', undefined, 'api-client')
        return this.request<T>(endpoint, {
          ...options,
          authToken: newToken,
          _retryCount: 1,
        })
      }
    }

    return Err(createUnauthorizedError(responseText || 'Unauthorized. Please log in again.'))
  }

  private async handleErrorResponse<T>(response: Response): AsyncResult<T, ApiError> {
    const contentType = response.headers.get('content-type')
    const isJson = contentType?.includes('application/json')

    let message: string
    if (isJson) {
      const data = await response.json()
      message = data.message || `Request failed with status ${response.status}`
    } else {
      const text = await response.text()
      message = text || `Request failed with status ${response.status}`
    }

    if (response.status >= 500) {
      return Err(createServerError(response.status, message))
    }

    return Err(createNetworkError(response.status, message))
  }

  private async parseJsonResponse<T>(response: Response): AsyncResult<T, ApiError> {
    const contentType = response.headers.get('content-type')
    const isJson = contentType?.includes('application/json')

    if (!isJson) {
      return Err(createNetworkError(response.status, `Expected JSON response but received: ${contentType}`))
    }

    const json = await response.json()
    const data = json.data !== undefined ? json.data : json
    return Ok(data as T)
  }

  async request<T>(
    endpoint: string,
    options: RequestOptions = {}
  ): AsyncResult<T, ApiError> {
    const {
      method = 'GET',
      headers = {},
      body,
      authToken,
    } = options

    const url = `${this.baseUrl}${endpoint}`

    const fetchOptions: RequestInit = {
      method,
      headers: {
        'Content-Type': 'application/json',
        ...headers,
      },
      credentials: 'include',
    }

    if (authToken) {
      fetchOptions.headers = {
        ...fetchOptions.headers,
        Authorization: authToken,
      }
    }

    if (body && method !== 'GET') {
      fetchOptions.body = JSON.stringify(body)
    }

    try {
      const response = await fetch(url, fetchOptions)

      if (response.status === 429) {
        return Err(createRateLimitError(60000))
      }

      if (response.status === 401) {
        return this.handle401Error(response, endpoint, options)
      }

      if (response.status === 403) {
        return Err(createForbiddenError('access', endpoint))
      }

      if (response.status === 204) {
        return Ok(undefined as T)
      }

      if (!response.ok) {
        return this.handleErrorResponse(response)
      }

      return this.parseJsonResponse(response)
    } catch (error) {
      if (error instanceof TypeError && error.message.includes('fetch')) {
        return Err(createNetworkError(0, 'Network error. Please check your connection and try again.'))
      }

      return Err(createUnknownError(error))
    }
  }

  async get<T>(endpoint: string, authToken?: string): AsyncResult<T, ApiError> {
    return this.request<T>(endpoint, { method: 'GET', authToken })
  }

  async post<T>(endpoint: string, body: unknown, authToken?: string): AsyncResult<T, ApiError> {
    return this.request<T>(endpoint, { method: 'POST', body, authToken })
  }

  async put<T>(endpoint: string, body: unknown, authToken?: string): AsyncResult<T, ApiError> {
    return this.request<T>(endpoint, { method: 'PUT', body, authToken })
  }

  async delete<T>(endpoint: string, authToken?: string): AsyncResult<T, ApiError> {
    return this.request<T>(endpoint, { method: 'DELETE', authToken })
  }

  async patch<T>(endpoint: string, body: unknown, authToken?: string): AsyncResult<T, ApiError> {
    return this.request<T>(endpoint, { method: 'PATCH', body, authToken })
  }
}

export const apiClient = new ApiClient()

export { ApiClient }
export type { RequestOptions }
