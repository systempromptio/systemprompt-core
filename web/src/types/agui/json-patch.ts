export type JsonPatchOperation =
  | { op: 'add'; path: string; value: unknown }
  | { op: 'remove'; path: string }
  | { op: 'replace'; path: string; value: unknown }
  | { op: 'move'; from: string; path: string }
  | { op: 'copy'; from: string; path: string }
  | { op: 'test'; path: string; value: unknown }

export function createAddOperation(path: string, value: unknown): JsonPatchOperation {
  return { op: 'add', path, value }
}

export function createRemoveOperation(path: string): JsonPatchOperation {
  return { op: 'remove', path }
}

export function createReplaceOperation(path: string, value: unknown): JsonPatchOperation {
  return { op: 'replace', path, value }
}

export function createMoveOperation(from: string, path: string): JsonPatchOperation {
  return { op: 'move', from, path }
}

export function createCopyOperation(from: string, path: string): JsonPatchOperation {
  return { op: 'copy', from, path }
}

export function createTestOperation(path: string, value: unknown): JsonPatchOperation {
  return { op: 'test', path, value }
}

export function applyJsonPatch<T extends object>(target: T, operations: JsonPatchOperation[]): T {
  let result = structuredClone(target)
  for (const operation of operations) {
    result = applySingleOperation(result, operation)
  }
  return result
}

function applySingleOperation<T extends object>(target: T, operation: JsonPatchOperation): T {
  const path = parsePath(operation.path)

  switch (operation.op) {
    case 'add':
      return setValueAtPath(target, path, operation.value)
    case 'remove':
      return removeValueAtPath(target, path)
    case 'replace':
      return setValueAtPath(target, path, operation.value)
    case 'move': {
      const fromPath = parsePath(operation.from)
      const value = getValueAtPath(target, fromPath)
      const withoutSource = removeValueAtPath(target, fromPath)
      return setValueAtPath(withoutSource, path, value)
    }
    case 'copy': {
      const fromPath = parsePath(operation.from)
      const value = getValueAtPath(target, fromPath)
      return setValueAtPath(target, path, structuredClone(value))
    }
    case 'test': {
      const currentValue = getValueAtPath(target, path)
      if (JSON.stringify(currentValue) !== JSON.stringify(operation.value)) {
        throw new Error(`JSON Patch test failed at path: ${operation.path}`)
      }
      return target
    }
  }
}

function parsePath(path: string): string[] {
  if (path === '') return []
  if (!path.startsWith('/')) throw new Error(`Invalid JSON Pointer: ${path}`)
  return path
    .slice(1)
    .split('/')
    .map((segment) => segment.replace(/~1/g, '/').replace(/~0/g, '~'))
}

function getValueAtPath(object: unknown, path: string[]): unknown {
  let current = object
  for (const segment of path) {
    if (current === null || current === undefined) return undefined
    current = (current as Record<string, unknown>)[segment]
  }
  return current
}

function setValueAtPath<T extends object>(object: T, path: string[], value: unknown): T {
  if (path.length === 0) return value as T
  const result = structuredClone(object)
  let current: Record<string, unknown> = result as Record<string, unknown>
  for (let i = 0; i < path.length - 1; i++) {
    const segment = path[i]
    if (!(segment in current)) {
      current[segment] = {}
    }
    current = current[segment] as Record<string, unknown>
  }
  current[path[path.length - 1]] = value
  return result
}

function removeValueAtPath<T extends object>(object: T, path: string[]): T {
  if (path.length === 0) return {} as T
  const result = structuredClone(object)
  let current: Record<string, unknown> = result as Record<string, unknown>
  for (let i = 0; i < path.length - 1; i++) {
    const segment = path[i]
    if (!(segment in current)) return result
    current = current[segment] as Record<string, unknown>
  }
  delete current[path[path.length - 1]]
  return result
}
