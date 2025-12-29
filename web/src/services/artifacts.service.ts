import type { Artifact as A2AArtifact } from '@a2a-js/sdk'
import type { Artifact } from '@/types/artifact'
import { toArtifact } from '@/types/artifact'
import type { AsyncResult, ApiError } from '@/types/core'
import { Err, createParseError } from '@/types/core'
import { apiClient } from './api-client'
import { logger } from '@/lib/logger'

class ArtifactsService {

  private validateArtifacts(artifacts: A2AArtifact[]): Artifact[] {
    return artifacts
      .map(artifact => {
        try {
          return toArtifact(artifact)
        } catch (error) {
          logger.warn('Skipping invalid artifact from API', error, 'artifacts-service')
          return undefined
        }
      })
      .filter((artifact): artifact is Artifact => artifact !== undefined)
  }

  async listArtifacts(
    authToken: string | undefined,
    limit?: number
  ): AsyncResult<readonly Artifact[], ApiError> {
    const params = new URLSearchParams()
    if (limit) params.append('limit', limit.toString())

    const queryString = params.toString()
    const endpoint = queryString ? `/artifacts?${queryString}` : '/artifacts'

    const result = await apiClient.get<A2AArtifact[]>(endpoint, authToken)

    if (!result.ok) {
      return result
    }

    const validatedArtifacts = this.validateArtifacts(result.value)
    return { ok: true, value: validatedArtifacts }
  }

  async listArtifactsByContext(
    contextId: string,
    authToken: string | undefined
  ): AsyncResult<readonly Artifact[], ApiError> {
    const result = await apiClient.get<A2AArtifact[]>(
      `/contexts/${contextId}/artifacts`,
      authToken
    )

    if (!result.ok) {
      return result
    }

    const validatedArtifacts = this.validateArtifacts(result.value)
    return { ok: true, value: validatedArtifacts }
  }

  async listArtifactsByTask(
    taskId: string,
    authToken: string | undefined
  ): AsyncResult<readonly Artifact[], ApiError> {
    const result = await apiClient.get<A2AArtifact[]>(
      `/tasks/${taskId}/artifacts`,
      authToken
    )

    if (!result.ok) {
      return result
    }

    const validatedArtifacts = this.validateArtifacts(result.value)
    return { ok: true, value: validatedArtifacts }
  }

  async getArtifact(
    artifactId: string,
    authToken: string | undefined
  ): AsyncResult<Artifact, ApiError> {
    const result = await apiClient.get<A2AArtifact>(
      `/artifacts/${artifactId}`,
      authToken
    )

    if (!result.ok) {
      return result
    }

    try {
      const validated = toArtifact(result.value)
      return { ok: true, value: validated }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Invalid artifact from API'
      return Err(createParseError(message, JSON.stringify(result.value)))
    }
  }
}

export const artifactsService = new ArtifactsService()
