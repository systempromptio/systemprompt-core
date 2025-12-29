import type { JSONSchema7 } from 'json-schema'
import type { FormValues, FieldValue } from './types'
import { getFieldType } from './types'

export function extractDefaults(schema: JSONSchema7): FormValues {
  const defaults: FormValues = {}

  if (!schema.properties) return defaults

  for (const [fieldName, property] of Object.entries(schema.properties)) {
    if (typeof property === 'boolean') continue
    const defaultValue = getDefaultForProperty(property)
    if (defaultValue !== undefined) {
      defaults[fieldName] = defaultValue
    }
  }

  return defaults
}

function hasExplicitDefault(property: JSONSchema7): boolean {
  return property.default !== undefined
}

function getDefaultForProperty(property: JSONSchema7): FieldValue | undefined {
  if (property.default !== undefined) {
    return property.default as FieldValue
  }

  const fieldType = getFieldType(property)

  switch (fieldType) {
    case 'string':
      if (property.enum && property.enum.length > 0) {
        return property.enum[0] as string
      }
      return undefined

    case 'number':
    case 'integer':
      if (property.minimum !== undefined) {
        return property.minimum
      }
      return undefined

    case 'boolean':
      return undefined

    case 'object':
      if (property.properties) {
        const nestedDefaults: Record<string, FieldValue> = {}
        let hasDefaults = false

        for (const [key, nestedProp] of Object.entries(property.properties)) {
          if (typeof nestedProp === 'boolean') continue
          const nestedDefault = getDefaultForProperty(nestedProp)
          if (nestedDefault !== undefined) {
            nestedDefaults[key] = nestedDefault
            hasDefaults = true
          }
        }

        return hasDefaults ? nestedDefaults : undefined
      }
      return undefined

    case 'array':
      if (property.default !== undefined) {
        return property.default as FieldValue
      }
      return undefined

    default:
      return undefined
  }
}

export function mergeWithDefaults(values: FormValues, schema: JSONSchema7): FormValues {
  const defaults = extractDefaults(schema)
  return {
    ...defaults,
    ...values,
  }
}

export function hasRequiredFields(schema: JSONSchema7): boolean {
  return Boolean(schema.required && schema.required.length > 0)
}

export function getRequiredFields(schema: JSONSchema7): string[] {
  return schema.required || []
}

export function canAutoSubmit(schema: JSONSchema7): boolean {
  if (!schema.required || schema.required.length === 0) {
    return true
  }

  if (!schema.properties) return false

  for (const requiredField of schema.required) {
    const property = schema.properties[requiredField]
    if (!property || typeof property === 'boolean' || !hasExplicitDefault(property)) {
      return false
    }
  }

  return true
}
