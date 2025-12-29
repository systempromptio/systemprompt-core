interface WebAuthnResponse<T = unknown> {
  data?: T
  error?: string
  error_description?: string
}

interface TokenResponse {
  access_token: string
  token_type: string
  expires_in: number
  refresh_token?: string
  scope?: string
}

interface OAuthParams {
  response_type?: string
  client_id?: string
  redirect_uri?: string
  scope?: string
  state?: string
  code_challenge?: string
  code_challenge_method?: string
}

interface AuthenticationStartResponse {
  publicKey: PublicKeyCredentialRequestOptionsJSON
  challenge_id: string
}

interface RegistrationStartResponse {
  publicKey: PublicKeyCredentialCreationOptionsJSON
}

interface RegistrationFinishResponse {
  user_id: string
}

interface AuthenticationFinishResponse {
  user_id: string
  oauth_state?: string
  success: boolean
}

class WebAuthnService {
  private baseUrl = '/api/v1/core/oauth'
  private codeVerifier: string
  private defaultOAuthParams: OAuthParams

  private oauthParamsToRecord(params: OAuthParams): Record<string, string> {
    const result: Record<string, string> = {}
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        result[key] = String(value)
      }
    })
    return result
  }

  constructor() {
    this.codeVerifier = this.generateRandomString(43)
    this.defaultOAuthParams = {
      response_type: 'code',
      client_id: 'sp_web',
      redirect_uri: window.location.origin + '/auth/callback',
      scope: 'user',
      state: this.generateRandomString(32),
      code_challenge: '',
      code_challenge_method: 'S256'
    }
    this.initializeCodeChallenge()
  }

  private generateRandomString(length: number): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~'
    let result = ''
    for (let i = 0; i < length; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length))
    }
    return result
  }

  private async initializeCodeChallenge() {
    this.defaultOAuthParams.code_challenge = await this.generateCodeChallenge(this.codeVerifier)
  }

  private async generateCodeChallenge(verifier: string): Promise<string> {
    const encoder = new TextEncoder()
    const data = encoder.encode(verifier)
    const hashBuffer = await crypto.subtle.digest('SHA-256', data)
    const hashArray = Array.from(new Uint8Array(hashBuffer))
    const base64String = btoa(String.fromCharCode(...hashArray))
    return base64String
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=/g, '')
  }

  private async makeRequest<T = unknown>(url: string, method: string, authHeader: string, body?: Record<string, unknown>): Promise<WebAuthnResponse<T>> {
    const options: RequestInit = {
      method,
      headers: {
        'Content-Type': 'application/json',
        'Authorization': authHeader
      }
    }

    if (body) {
      options.body = JSON.stringify(body)
    }

    try {
      const response = await fetch(url, options)
      const data = await response.json()

      if (!response.ok) {
        throw new Error(data.error_description || data.error || 'Request failed')
      }

      return {
        data,
        error: undefined
      }
    } catch (error) {
      return {
        data: undefined,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      }
    }
  }


  async registerPasskey(
    username: string,
    email: string,
    authHeader: string,
    fullName?: string
  ): Promise<{ success: boolean; error?: string; userId?: string }> {
    try {
      const params = new URLSearchParams({
        username,
        email,
        ...(fullName && { full_name: fullName })
      })

      const startUrl = `${this.baseUrl}/webauthn/register/start?${params}`
      const response = await fetch(startUrl, {
        method: 'POST',
        headers: {
          'Authorization': authHeader
        }
      })

      if (!response.ok) {
        const error = await response.json()
        throw new Error(error.error_description || error.error || 'Failed to start registration')
      }

      const startData: RegistrationStartResponse = await response.json()
      const challengeId = response.headers.get('x-challenge-id')

      if (!challengeId) {
        throw new Error('No challenge ID received from server')
      }

      const publicKeyOptions = PublicKeyCredential.parseCreationOptionsFromJSON(startData.publicKey)

      const credential = await navigator.credentials.create({
        publicKey: publicKeyOptions
      })

      if (!credential || !(credential instanceof PublicKeyCredential)) {
        throw new Error('Passkey creation was cancelled')
      }

      const finishResponse = await this.makeRequest<RegistrationFinishResponse>(
        `${this.baseUrl}/webauthn/register/finish`,
        'POST',
        authHeader,
        {
          challenge_id: challengeId,
          username,
          email,
          ...(fullName && { full_name: fullName }),
          credential: credential.toJSON()
        }
      )

      if (finishResponse.error || !finishResponse.data) {
        throw new Error(finishResponse.error || 'Failed to complete registration')
      }

      return {
        success: true,
        userId: finishResponse.data.user_id
      }
    } catch (error) {
      if (error instanceof Error) {
        if (error.name === 'NotAllowedError') {
          return { success: false, error: 'Passkey creation was cancelled or failed' }
        } else if (error.name === 'NotSupportedError') {
          return { success: false, error: 'WebAuthn is not supported on this device' }
        }
        return { success: false, error: error.message }
      }
      return { success: false, error: 'Unknown error occurred' }
    }
  }

  async authenticateWithPasskey(email: string, authHeader: string): Promise<{ success: boolean; error?: string; accessToken?: string; refreshToken?: string; expiresIn?: number }> {
    try {
      const authParams = new URLSearchParams({
        email: email,
        oauth_state: JSON.stringify(this.defaultOAuthParams)
      })

      const startResponse = await fetch(
        `${this.baseUrl}/webauthn/auth/start?${authParams}`,
        {
          method: 'POST',
          headers: {
            'Authorization': authHeader
          }
        }
      )

      if (!startResponse.ok) {
        const error = await startResponse.json()
        throw new Error(error.error_description || error.error || 'Failed to start authentication')
      }

      const startData: AuthenticationStartResponse = await startResponse.json()
      const challengeId = startData.challenge_id

      if (!challengeId) {
        throw new Error('No challenge ID received from server')
      }

      const publicKeyOptions = PublicKeyCredential.parseRequestOptionsFromJSON(startData.publicKey)

      const credential = await navigator.credentials.get({
        publicKey: publicKeyOptions
      })

      if (!credential || !(credential instanceof PublicKeyCredential)) {
        throw new Error('Authentication was cancelled')
      }

      const finishResponse = await this.makeRequest<AuthenticationFinishResponse>(
        `${this.baseUrl}/webauthn/auth/finish`,
        'POST',
        authHeader,
        {
          challenge_id: challengeId,
          credential: credential.toJSON()
        }
      )

      if (finishResponse.error || !finishResponse.data) {
        throw new Error(finishResponse.error || 'Failed to complete authentication')
      }

      const authFinishData: AuthenticationFinishResponse = finishResponse.data

      const completeParams = new URLSearchParams({
        user_id: authFinishData.user_id,
        ...this.oauthParamsToRecord(this.defaultOAuthParams)
      })

      const completeResponse = await fetch(
        `${this.baseUrl}/webauthn/complete?${completeParams}`,
        {
          method: 'GET',
          headers: {
            'Content-Type': 'application/json',
          }
        }
      )

      if (!completeResponse.ok) {
        const error = await completeResponse.json()
        throw new Error(error.error_description || error.error || 'OAuth completion failed')
      }

      const completeData = await completeResponse.json()
      const code = completeData.authorization_code

      if (!code) {
        throw new Error('No authorization code received')
      }

      const tokenResponse = await this.exchangeCodeForToken(code)

      return {
        success: true,
        accessToken: tokenResponse.access_token,
        refreshToken: tokenResponse.refresh_token,
        expiresIn: tokenResponse.expires_in
      }
    } catch (error) {
      if (error instanceof Error) {
        if (error.name === 'NotAllowedError') {
          return { success: false, error: 'Authentication was cancelled or failed' }
        } else if (error.name === 'NotSupportedError') {
          return { success: false, error: 'WebAuthn is not supported on this device' }
        }
        return { success: false, error: error.message }
      }
      return { success: false, error: 'Unknown error occurred' }
    }
  }

  private async exchangeCodeForToken(code: string): Promise<TokenResponse> {
    const tokenParams = new URLSearchParams({
      grant_type: 'authorization_code',
      code: code,
      client_id: this.defaultOAuthParams.client_id!,
      redirect_uri: this.defaultOAuthParams.redirect_uri!,
      code_verifier: this.codeVerifier
    })

    const response = await fetch(`${this.baseUrl}/token`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      body: tokenParams.toString()
    })

    if (!response.ok) {
      const error = await response.json()
      throw new Error(error.error_description || error.error || 'Failed to exchange code for token')
    }

    return await response.json()
  }

  isWebAuthnSupported(): boolean {
    return !!window.PublicKeyCredential
  }
}

export const webAuthnService = new WebAuthnService()