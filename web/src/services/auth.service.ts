import { ApiClient } from './api-client'
import type { ApiError } from '@/types/core'

function getErrorMessage(error: ApiError): string {
  if ('message' in error) {
    return error.message
  }
  if (error.kind === 'timeout') {
    return 'Request timed out'
  }
  if (error.kind === 'rate_limit') {
    return 'Rate limited, please try again later'
  }
  if (error.kind === 'forbidden') {
    return `Access denied: ${error.action} on ${error.resource}`
  }
  return 'An unknown error occurred'
}

interface AnonymousTokenResponse {
  access_token: string
  token_type: string
  expires_in: number
  session_id: string
  user_id: string
}

interface TokenResponse {
  access_token: string
  token_type: string
  expires_in: number
  refresh_token?: string
  scope?: string
}

class AuthService {
  private oauthClient: ApiClient

  constructor() {
    this.oauthClient = new ApiClient('/api/v1/core/oauth')
  }

  async generateAnonymousToken(): Promise<{
    token?: AnonymousTokenResponse
    error?: string
  }> {
    const result = await this.oauthClient.post<AnonymousTokenResponse>(
      '/session',
      {}
    )

    if (result.ok) {
      return { token: result.value }
    }
    return { error: getErrorMessage(result.error) }
  }

  async refreshAccessToken(refreshToken: string): Promise<{
    token?: TokenResponse
    error?: string
  }> {
    const body = {
      grant_type: 'refresh_token',
      refresh_token: refreshToken,
    }

    const result = await this.oauthClient.post<TokenResponse>(
      '/token',
      body
    )

    if (result.ok) {
      return { token: result.value }
    }
    return { error: getErrorMessage(result.error) }
  }
}

export const authService = new AuthService()
