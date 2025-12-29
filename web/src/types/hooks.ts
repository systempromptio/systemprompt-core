export interface AsyncHookState<T> {
  data: T | undefined
  loading: boolean
  error: string | undefined
}

export interface ConnectionHookState {
  isConnected: boolean
  isConnecting: boolean
  error: Error | undefined
}

export interface AuthHookState {
  isAuthenticated: boolean
  email: string | undefined
  username: string | undefined
  scopes: string[]
  userType: string | undefined
}

export interface ExecutionHookState<T = unknown> {
  status: 'idle' | 'pending' | 'success' | 'error'
  result: T | undefined
  error: string | undefined
  isExecuting: boolean
}

export interface ModalHookState<T = unknown> {
  isOpen: boolean
  data: T | undefined
  open: (data?: T) => void
  close: () => void
}

export interface PaginationHookState<T> {
  items: T[]
  page: number
  total: number
  pageSize: number
  hasNext: boolean
  hasPrev: boolean
}

export interface FilterHookState<T, F> {
  items: T[]
  filters: F
  setFilters: (filters: Partial<F>) => void
  resetFilters: () => void
  total: number
}
