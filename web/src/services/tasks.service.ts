import type { Task } from '@/types/task'
import type { AsyncResult, ApiError } from '@/types/core'
import { apiClient } from './api-client'

class TasksService {

  async listTasksByContext(
    contextId: string,
    authToken: string | undefined
  ): AsyncResult<readonly Task[], ApiError> {
    const result = await apiClient.get<Task[]>(
      `/contexts/${contextId}/tasks`,
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async getTask(
    taskId: string,
    authToken: string | undefined
  ): AsyncResult<Task, ApiError> {
    const result = await apiClient.get<Task>(
      `/tasks/${taskId}`,
      authToken
    )
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }

  async listTasks(
    authToken: string | undefined,
    status?: string,
    limit?: number
  ): AsyncResult<readonly Task[], ApiError> {
    const params = new URLSearchParams()
    if (status) params.append('status', status)
    if (limit) params.append('limit', limit.toString())

    const queryString = params.toString()
    const endpoint = queryString ? `/tasks?${queryString}` : '/tasks'

    const result = await apiClient.get<Task[]>(endpoint, authToken)
    if (!result.ok) {
      return result
    }
    return { ok: true, value: result.value }
  }
}

export const tasksService = new TasksService()
