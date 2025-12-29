interface JWTClaims {
  sub?: string
  email?: string
  session_id?: string
  scope?: string
  user_type?: string
  username?: string
  exp?: number
  iat?: number
  [key: string]: unknown
}

export function decodeJWT(token: string): JWTClaims {
  try {
    const parts = token.split('.')
    if (parts.length !== 3) {
      throw new Error('Invalid JWT format: expected 3 parts separated by dots')
    }

    const payload = parts[1]
    const decoded = atob(payload.replace(/-/g, '+').replace(/_/g, '/'))
    const claims = JSON.parse(decoded) as JWTClaims

    return claims
  } catch (error) {
    throw new Error(
      `Failed to decode JWT: ${error instanceof Error ? error.message : 'Unknown error'}`
    )
  }
}

export function extractUserIdFromJWT(token: string): string {
  const claims = decodeJWT(token)
  if (!claims.sub) {
    throw new Error('JWT missing required "sub" claim (user ID)')
  }
  return claims.sub
}

export function extractSessionIdFromJWT(token: string): string {
  const claims = decodeJWT(token)
  if (!claims.session_id) {
    throw new Error('JWT missing required "session_id" claim')
  }
  return claims.session_id
}

export function extractEmailFromJWT(token: string): string {
  const claims = decodeJWT(token)
  if (!claims.email) {
    throw new Error('JWT missing required "email" claim')
  }
  return claims.email
}

export function extractScopesFromJWT(token: string): string[] {
  const claims = decodeJWT(token)
  return claims.scope ? claims.scope.split(' ').filter((s) => s.length > 0) : []
}

export function extractUserTypeFromJWT(token: string): string | undefined {
  const claims = decodeJWT(token)
  return claims.user_type || undefined
}

export function extractUsernameFromJWT(token: string): string | undefined {
  const claims = decodeJWT(token)
  return claims.username || undefined
}
