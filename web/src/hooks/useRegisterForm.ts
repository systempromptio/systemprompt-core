import { useState, useCallback } from 'react'
import { useAuthStore } from '@/stores/auth.store'
import { extractUserIdFromJWT, extractEmailFromJWT } from '@/utils/jwt'

export function useRegisterForm() {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [fullName, setFullName] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | undefined>(undefined)
  const [success, setSuccess] = useState<string | undefined>(undefined)

  const {
    setAuth,
    isWebAuthnSupported: checkWebAuthnSupported,
    registerPasskey,
    authenticateWithPasskey
  } = useAuthStore()

  const [isWebAuthnSupported] = useState(checkWebAuthnSupported())

  const reset = useCallback(() => {
    setUsername('')
    setEmail('')
    setFullName('')
    setError(undefined)
    setSuccess(undefined)
    setLoading(false)
  }, [])

  const validateUsername = (username: string): { valid: boolean; error?: string } => {
    if (!username) return { valid: false, error: 'Username is required' }
    if (username.length < 3) return { valid: false, error: 'Username must be at least 3 characters' }
    if (username.length > 50) return { valid: false, error: 'Username must be less than 50 characters' }
    if (!/^[a-zA-Z0-9_-]+$/.test(username)) {
      return { valid: false, error: 'Username can only contain letters, numbers, underscores, and hyphens' }
    }
    return { valid: true }
  }

  const validateEmail = (email: string): { valid: boolean; error?: string } => {
    if (!email) return { valid: false, error: 'Email is required' }
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      return { valid: false, error: 'Please enter a valid email address' }
    }
    return { valid: true }
  }

  const handleRegister = async (onSuccess?: () => void) => {
    const usernameValidation = validateUsername(username)
    if (!usernameValidation.valid) {
      setError(usernameValidation.error || 'Invalid username')
      return
    }

    const emailValidation = validateEmail(email)
    if (!emailValidation.valid) {
      setError(emailValidation.error || 'Invalid email')
      return
    }

    setLoading(true)
    setError(undefined)
    setSuccess(undefined)

    const registerResult = await registerPasskey(username, email, fullName || undefined)

    if (registerResult.success) {
      setSuccess('Passkey created successfully! Now signing you in...')

      const authResult = await authenticateWithPasskey(email)

      if (authResult.success && authResult.accessToken) {
        try {
          const userId = extractUserIdFromJWT(authResult.accessToken)
          const userEmail = extractEmailFromJWT(authResult.accessToken)
          setSuccess('Registration complete! You are now signed in.')
          setAuth(userEmail, userId, authResult.accessToken, authResult.refreshToken, authResult.expiresIn || 3600)

          setTimeout(() => {
            onSuccess?.()
          }, 1500)
        } catch (jwtError) {
          setError(`Invalid authentication token: ${jwtError instanceof Error ? jwtError.message : 'Unknown error'}`)
        }
      } else {
        setError('Registration succeeded but sign-in failed. Please try signing in manually.')
      }
    } else {
      const errorMsg = registerResult.error || 'Registration failed'
      if (errorMsg.toLowerCase().includes('already') || errorMsg.toLowerCase().includes('exist')) {
        setError(`${errorMsg}. Try signing in instead?`)
      } else {
        setError(errorMsg)
      }
    }

    setLoading(false)
  }

  return {
    username,
    setUsername,
    email,
    setEmail,
    fullName,
    setFullName,
    loading,
    error,
    success,
    isWebAuthnSupported,
    reset,
    handleRegister,
  }
}
