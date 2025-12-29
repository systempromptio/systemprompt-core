import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { extractScopesFromJWT, extractUserTypeFromJWT, extractUsernameFromJWT, extractSessionIdFromJWT } from '@/utils/jwt'
import { resetAllStores, clearUserLocalStorage } from './reset'
import { UIStateKey } from '@/constants'
import { webAuthnService } from '@/services/webauthn.service'

export interface AuthUser {
  userId: string
  email: string | undefined
  username: string
  sessionId: string
  userType: string
  scopes: readonly string[]
  accessToken: string
  refreshToken: string | undefined
  tokenExpiry: number
}

interface AuthState {
  isAuthenticated: boolean
  email: string | undefined
  userId: string | undefined
  sessionId: string | undefined
  username: string | undefined
  scopes: readonly string[]
  userType: string | undefined
  accessToken: string | undefined
  refreshToken: string | undefined
  tokenExpiry: number | undefined

  showAuthModal: boolean
  authAgentName: string | undefined
  authCallback: (() => void) | undefined

  setAuth: (email: string, userId: string, accessToken: string, refreshToken: string | undefined, expiresIn: number) => void
  updateTokens: (accessToken: string, refreshToken: string | undefined, expiresIn: number) => void
  setAnonymousAuth: (accessToken: string, userId: string, sessionId: string, expiresIn: number) => void
  clearAuth: () => void
  clearAuthAndRestoreAnonymous: () => Promise<void>
  isTokenValid: () => boolean
  getAuthHeader: () => string | undefined

  openAuthModal: (agentName?: string, callback?: () => void) => void
  closeAuthModal: () => void
  executeAuthCallback: () => void
  isWebAuthnSupported: () => boolean
  authenticateWithPasskey: (email: string) => Promise<{ success: boolean; error?: string; accessToken?: string; refreshToken?: string; expiresIn?: number }>
  registerPasskey: (username: string, email: string, fullName?: string) => Promise<{ success: boolean; error?: string; userId?: string }>
}

const TOKEN_EXPIRY_BUFFER_MS = 30000

const extractTokenData = (accessToken: string) => ({
  sessionId: extractSessionIdFromJWT(accessToken),
  username: extractUsernameFromJWT(accessToken),
  scopes: extractScopesFromJWT(accessToken) as readonly string[],
  userType: extractUserTypeFromJWT(accessToken),
})

const clearAuthState = () => ({
  isAuthenticated: false,
  email: undefined,
  userId: undefined,
  sessionId: undefined,
  username: undefined,
  scopes: [] as readonly string[],
  userType: undefined,
  accessToken: undefined,
  refreshToken: undefined,
  tokenExpiry: undefined,
})

const handleUserSwitch = (previousUserId: string | undefined) => {
  clearUserLocalStorage(previousUserId)
  resetAllStores()
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      isAuthenticated: false,
      email: undefined,
      userId: undefined,
      sessionId: undefined,
      username: undefined,
      scopes: [] as readonly string[],
      userType: undefined,
      accessToken: undefined,
      refreshToken: undefined,
      tokenExpiry: undefined,

      showAuthModal: false,
      authAgentName: undefined,
      authCallback: undefined,

      setAuth: (email, userId, accessToken, refreshToken, expiresIn) => {
        const previousUserId = get().userId
        const tokenData = extractTokenData(accessToken)

        set({
          isAuthenticated: true,
          email,
          userId,
          ...tokenData,
          accessToken,
          refreshToken,
          tokenExpiry: Date.now() + (expiresIn * 1000),
        })

        if (previousUserId !== userId) {
          handleUserSwitch(previousUserId)
        }
      },

      updateTokens: (accessToken, refreshToken, expiresIn) => {
        const tokenData = extractTokenData(accessToken)
        set({
          ...tokenData,
          accessToken,
          refreshToken,
          tokenExpiry: Date.now() + (expiresIn * 1000),
        })
      },

      setAnonymousAuth: (accessToken, userId, sessionId, expiresIn) => {
        const previousUserId = get().userId
        const tokenData = extractTokenData(accessToken)

        set({
          isAuthenticated: true,
          email: undefined,
          userId,
          sessionId,
          username: tokenData.username || 'Anonymous',
          scopes: tokenData.scopes,
          userType: tokenData.userType || 'anon',
          accessToken,
          tokenExpiry: Date.now() + (expiresIn * 1000),
        })

        if (previousUserId !== userId) {
          handleUserSwitch(previousUserId)
        }
      },

      clearAuth: () => {
        const previousUserId = get().userId
        set(clearAuthState())
        handleUserSwitch(previousUserId)
      },

      clearAuthAndRestoreAnonymous: async () => {
        const previousUserId = get().userId
        set(clearAuthState())
        handleUserSwitch(previousUserId)

        const { authService } = await import('@/services/auth.service')
        const { token, error } = await authService.generateAnonymousToken()

        if (error || !token) return

        const tokenData = extractTokenData(token.access_token)
        set({
          isAuthenticated: true,
          email: undefined,
          userId: token.user_id,
          sessionId: token.session_id,
          username: tokenData.username || 'Anonymous',
          scopes: tokenData.scopes,
          userType: tokenData.userType || 'anon',
          accessToken: token.access_token,
          tokenExpiry: Date.now() + (token.expires_in * 1000),
        })
      },

      isTokenValid: () => {
        const state = get()
        if (!state.accessToken || !state.tokenExpiry) return false
        return Date.now() < (state.tokenExpiry - TOKEN_EXPIRY_BUFFER_MS)
      },

      getAuthHeader: () => {
        const state = get()
        return state.isAuthenticated && state.accessToken && state.isTokenValid()
          ? `Bearer ${state.accessToken}`
          : undefined
      },

      openAuthModal: (agentName, callback) => {
        set({
          showAuthModal: true,
          authAgentName: agentName,
          authCallback: callback,
        })
      },

      closeAuthModal: () => {
        set({
          showAuthModal: false,
          authAgentName: undefined,
          authCallback: undefined,
        })
      },

      executeAuthCallback: () => {
        const { authCallback } = get()
        authCallback?.()
        get().closeAuthModal()
      },

      isWebAuthnSupported: () => {
        return webAuthnService.isWebAuthnSupported()
      },

      authenticateWithPasskey: async (email: string) => {
        const authHeader = get().getAuthHeader()
        if (!authHeader) {
          return { success: false, error: 'No authentication token available. Please refresh the page.' }
        }
        return webAuthnService.authenticateWithPasskey(email, authHeader)
      },

      registerPasskey: async (username: string, email: string, fullName?: string) => {
        const authHeader = get().getAuthHeader()
        if (!authHeader) {
          return { success: false, error: 'No authentication token available. Please refresh the page.' }
        }
        return webAuthnService.registerPasskey(username, email, authHeader, fullName)
      },
    }),
    {
      name: UIStateKey.AUTH_STORAGE,
      partialize: (state) => ({
        isAuthenticated: state.isAuthenticated,
        email: state.email,
        userId: state.userId,
        sessionId: state.sessionId,
        username: state.username,
        scopes: state.scopes,
        userType: state.userType,
        accessToken: state.accessToken,
        refreshToken: state.refreshToken,
        tokenExpiry: state.tokenExpiry,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) return

        if (!Array.isArray(state.scopes)) {
          state.scopes = []
        }

        const BUFFER_MS = 30000
        if (state.tokenExpiry && Date.now() >= (state.tokenExpiry - BUFFER_MS)) {
          state.clearAuth()
        }
      },
    }
  )
)

export const authSelectors = {
  getCurrentUser: (state: AuthState): AuthUser | undefined => {
    if (!state.isAuthenticated || !state.userId || !state.accessToken || !state.tokenExpiry) {
      return undefined
    }
    return {
      userId: state.userId,
      email: state.email,
      username: state.username ?? 'Anonymous',
      sessionId: state.sessionId ?? '',
      userType: state.userType ?? 'user',
      scopes: state.scopes,
      accessToken: state.accessToken,
      refreshToken: state.refreshToken,
      tokenExpiry: state.tokenExpiry,
    }
  },

  isAuthenticated: (state: AuthState): boolean => state.isAuthenticated,

  hasValidToken: (state: AuthState): boolean => {
    if (!state.isAuthenticated || !state.accessToken || !state.tokenExpiry) {
      return false
    }
    return Date.now() < (state.tokenExpiry - TOKEN_EXPIRY_BUFFER_MS)
  },

  getScopes: (state: AuthState): readonly string[] => state.scopes,

  getUserId: (state: AuthState): string | undefined => state.userId,

  getUsername: (state: AuthState): string | undefined => state.username,

  getUserType: (state: AuthState): string | undefined => state.userType,
}