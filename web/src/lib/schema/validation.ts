import type { JSONSchema7 } from 'json-schema'

export function isJSONSchema7(value: unknown): value is JSONSchema7 {
  if (typeof value !== 'object' || value === null) {
    return false
  }

  const schema = value as Record<string, unknown>

  if (!('type' in schema)) {
    return false
  }

  const type = schema.type
  if (type !== 'object' && !(Array.isArray(type) && type.includes('object'))) {
    return false
  }

  if ('properties' in schema && typeof schema.properties !== 'object') {
    return false
  }

  if ('required' in schema) {
    if (!Array.isArray(schema.required)) {
      return false
    }
    if (!schema.required.every((item) => typeof item === 'string')) {
      return false
    }
  }

  return true
}

export function validateAndCastSchema(
  rawSchema: unknown,
  context?: string
): JSONSchema7 {
  const contextPrefix = context ? `[${context}] ` : ''

  if (!isJSONSchema7(rawSchema)) {
    const errorMsg = `${contextPrefix}Invalid JSON Schema: Expected object with type='object'`
    throw new SchemaValidationError(errorMsg, rawSchema)
  }

  const schema = rawSchema as JSONSchema7


  return schema
}

export class SchemaValidationError extends Error {
  readonly invalidSchema: unknown

  constructor(message: string, invalidSchema: unknown) {
    super(message)
    this.name = 'SchemaValidationError'
    this.invalidSchema = invalidSchema
  }
}

export function extractToolInputSchema(
  inputSchema: unknown,
  toolName: string
): JSONSchema7 {
  try {
    return validateAndCastSchema(inputSchema, `Tool: ${toolName}`)
  } catch {
    return {
      type: 'object',
      properties: {},
      required: [],
    }
  }
}
