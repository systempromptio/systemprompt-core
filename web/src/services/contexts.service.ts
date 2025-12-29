import type { AsyncResult, ApiError } from '@/types/core'
import { Err, createNetworkError } from '@/types/core'
import { apiClient } from './api-client'

const INVALID_CONTEXT_IDS = ['undefined', 'null', '', '__CONTEXT_LOADING__']

function isValidContextId(contextId: string | undefined | null): contextId is string {
  if (!contextId) return false
  if (INVALID_CONTEXT_IDS.includes(contextId)) return false
  return true
}

function createInvalidContextIdError(): ApiError {
  return createNetworkError(400, 'Invalid context ID. Please select or create a valid conversation first.')
}

export interface UserContext {
  context_id: string
  user_id: string
  name: string
  created_at: string
  updated_at: string
}

export interface UserContextWithStats extends UserContext {
  task_count: number
  message_count: number
  last_message_at: string | undefined
}

class ContextsService {

  async listContexts(authToken: string | undefined): AsyncResult<readonly UserContextWithStats[], ApiError> {
    const result = await apiClient.get<UserContextWithStats[]>(
      '/contexts',
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async createContext(
    name: string,
    authToken: string | undefined
  ): AsyncResult<UserContext, ApiError> {
    const result = await apiClient.post<UserContext>(
      '/contexts',
      { name },
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async getContext(
    contextId: string,
    authToken: string | undefined
  ): AsyncResult<UserContext, ApiError> {
    if (!isValidContextId(contextId)) {
      return Err(createInvalidContextIdError())
    }
    const result = await apiClient.get<UserContext>(
      `/contexts/${contextId}`,
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async updateContext(
    contextId: string,
    name: string,
    authToken: string | undefined
  ): AsyncResult<UserContext, ApiError> {
    if (!isValidContextId(contextId)) {
      return Err(createInvalidContextIdError())
    }
    const result = await apiClient.put<UserContext>(
      `/contexts/${contextId}`,
      { name },
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async deleteContext(
    contextId: string,
    authToken: string | undefined
  ): AsyncResult<void, ApiError> {
    if (!isValidContextId(contextId)) {
      return Err(createInvalidContextIdError())
    }
    const result = await apiClient.delete<void>(
      `/contexts/${contextId}`,
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: undefined }
  }
}

export const contextsService = new ContextsService()
