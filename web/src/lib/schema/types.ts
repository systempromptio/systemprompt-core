import type { JSONSchema7, JSONSchema7Definition } from 'json-schema'

export type JsonSchema = JSONSchema7
export type JsonSchemaProperty = JSONSchema7Definition

export interface DataSourceConfig {
  tool: string
  action: string
  value_field: string
  label_field?: string
  label_template?: string
  filter?: Record<string, unknown>
  cache_object?: boolean
}

export interface ExtendedJSONSchema7 extends JSONSchema7 {
  'x-data-source'?: DataSourceConfig
}

export type FieldValue = string | number | boolean | null | Record<string, unknown> | unknown[] | undefined

export type FormValues = Record<string, FieldValue>

export type { ValidationError } from '@/types/core/errors'
export type { FieldValidationError }

interface FieldValidationError {
  field: string
  message: string
}

export interface ValidationResult {
  valid: boolean
  errors: Record<string, string>
}

export function getEnumValues(property: JSONSchema7): unknown[] {
  return property.enum || []
}

export function isRequired(fieldName: string, schema: JSONSchema7): boolean {
  return Array.isArray(schema.required) && schema.required.includes(fieldName)
}

export function getFieldType(property: JSONSchema7): string {
  if (Array.isArray(property.type)) {
    const nonNullType = property.type.find(t => t !== 'null')
    return nonNullType || property.type[0] || 'string'
  }
  return property.type || 'string'
}

export function isEnumField(property: JSONSchema7): boolean {
  return Array.isArray(property.enum) && property.enum.length > 0
}

export function getDefaultValue(property: JSONSchema7): FieldValue | undefined {
  return property.default as FieldValue | undefined
}

export function getNestedValue(obj: FormValues, path: string): FieldValue {
  const parts = path.split('.')
  let current: unknown = obj

  for (const part of parts) {
    if (current === undefined || current === null || typeof current !== 'object') {
      return undefined
    }
    current = (current as Record<string, unknown>)[part]
  }

  return current as FieldValue
}

export function setNestedValue(obj: FormValues, path: string, value: FieldValue): FormValues {
  const parts = path.split('.')
  const result = { ...obj }
  let current: Record<string, unknown> = result

  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i]

    if (current[part] === undefined || typeof current[part] !== 'object' || Array.isArray(current[part])) {
      current[part] = {}
    } else {
      current[part] = { ...(current[part] as Record<string, unknown>) }
    }

    current = current[part] as Record<string, unknown>
  }

  current[parts[parts.length - 1]] = value

  return result
}

export function flattenErrors(errors: Record<string, unknown>, prefix = ''): Record<string, string> {
  const flattened: Record<string, string> = {}

  for (const [key, value] of Object.entries(errors)) {
    const fullKey = prefix ? `${prefix}.${key}` : key

    if (typeof value === 'string') {
      flattened[fullKey] = value
    } else if (typeof value === 'object' && value !== null) {
      Object.assign(flattened, flattenErrors(value as Record<string, unknown>, fullKey))
    }
  }

  return flattened
}
