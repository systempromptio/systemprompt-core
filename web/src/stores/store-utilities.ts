export const getStorageKey = (key: string, userId?: string): string => {
  return userId ? `${key}:${userId}` : key
}

export const loadFromStorage = (key: string): string | undefined => {
  if (typeof window === 'undefined') return undefined
  try {
    const value = localStorage.getItem(key)
    return value ?? undefined
  } catch {
    return undefined
  }
}

export const saveToStorage = (key: string, value: string): void => {
  if (typeof window === 'undefined') return
  try {
    localStorage.setItem(key, value)
  } catch {
    /* localStorage may not be available in some contexts */
  }
}

export const setError = (error: string) => ({
  error,
})

export const clearError = () => ({
  error: undefined,
})

export const ensureInArray = <T,>(item: T, array: readonly T[]): readonly T[] => {
  if (array.includes(item)) {
    return array
  }
  return [...array, item]
}

export const addToMapping = <K extends string | number | symbol, V>(
  mapping: Record<K, readonly V[]>,
  key: K,
  value: V
): void => {
  if (!mapping[key]) {
    mapping[key] = [value]
  } else if (!mapping[key].includes(value)) {
    mapping[key] = [...mapping[key], value]
  }
}

export const cloneRecordArrays = <K extends string | number | symbol, V>(
  record: Readonly<Record<K, readonly V[]>>
): Record<K, readonly V[]> => {
  const cloned: Record<K, readonly V[]> = {} as Record<K, readonly V[]>
  for (const key in record) {
    cloned[key] = [...record[key]]
  }
  return cloned
}

export const openPersisted = <T,>(selectedId: string) => ({
  selectedId,
  ephemeralItem: undefined as T | undefined,
})

export const openEphemeral = <T,>(item: T) => ({
  selectedId: undefined as string | undefined,
  ephemeralItem: item,
})

export const closeModal = <T,>() => ({
  selectedId: undefined as string | undefined,
  ephemeralItem: undefined as T | undefined,
})
