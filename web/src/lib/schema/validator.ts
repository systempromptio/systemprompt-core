import type { JSONSchema7 } from 'json-schema'
import type {
  FormValues,
  ValidationResult,
  FieldValue,
} from './types'
import { getFieldType, isRequired } from './types'

export function validateAgainstSchema(
  values: FormValues,
  schema: JSONSchema7
): ValidationResult {
  const errors: Record<string, string> = {}

  if (schema.required) {
    for (const fieldName of schema.required) {
      const value = values[fieldName]
      if (value === undefined || value === null || value === '') {
        errors[fieldName] = 'This field is required'
      }
    }
  }

  if (schema.properties) {
    for (const [fieldName, property] of Object.entries(schema.properties)) {
      if (typeof property === 'boolean') continue

      const value = values[fieldName]

      if (!isRequired(fieldName, schema) && (value === undefined || value === null || value === '')) {
        continue
      }

      if (errors[fieldName]) {
        continue
      }

      const fieldType = getFieldType(property)

      if (fieldType === 'object') {
        validateObject(value, property, fieldName, schema, errors)
      } else {
        const fieldError = validateField(fieldName, value, property)
        if (fieldError) {
          errors[fieldName] = fieldError
        }
      }
    }
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
  }
}

function validateField(
  _fieldName: string,
  value: FieldValue,
  property: JSONSchema7
): string | null {
  const fieldType = getFieldType(property)

  if (property.enum && Array.isArray(property.enum)) {
    const matchesEnum = property.enum.some(enumVal => {
      if (enumVal === value) return true
      if (String(enumVal) === String(value)) return true
      return false
    })

    if (!matchesEnum) {
      return `Must be one of: ${property.enum.join(', ')}`
    }
  }

  switch (fieldType) {
    case 'string':
      return validateString(value, property)
    case 'number':
    case 'integer':
      return validateNumber(value, property, fieldType)
    case 'boolean':
      return validateBoolean(value)
    case 'object':
      return null
    case 'array':
      return validateArray(value, property)
    default:
      return null
  }
}

function validateFormat(value: string, format: string): string | null {
  switch (format) {
    case 'email': {
      const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
      return emailRegex.test(value) ? null : 'Invalid email address'
    }

    case 'uri':
    case 'url':
      try {
        new URL(value)
        return null
      } catch {
        return 'Invalid URL'
      }

    case 'uuid': {
      const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
      return uuidRegex.test(value) ? null : 'Invalid UUID'
    }

    case 'date': {
      const dateRegex = /^\d{4}-\d{2}-\d{2}$/
      return dateRegex.test(value) && !isNaN(Date.parse(value)) ? null : 'Invalid date (use YYYY-MM-DD)'
    }

    case 'date-time':
      return !isNaN(Date.parse(value)) ? null : 'Invalid date-time'

    case 'time': {
      const timeRegex = /^\d{2}:\d{2}(:\d{2})?$/
      return timeRegex.test(value) ? null : 'Invalid time (use HH:MM or HH:MM:SS)'
    }

    case 'ipv4': {
      const ipv4Regex = /^(\d{1,3}\.){3}\d{1,3}$/
      return ipv4Regex.test(value) ? null : 'Invalid IPv4 address'
    }

    case 'ipv6': {
      const ipv6Regex = /^([0-9a-f]{0,4}:){7}[0-9a-f]{0,4}$/i
      return ipv6Regex.test(value) ? null : 'Invalid IPv6 address'
    }

    case 'hostname': {
      const hostnameRegex = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*$/i
      return hostnameRegex.test(value) ? null : 'Invalid hostname'
    }

    default:
      return null
  }
}

function validateString(value: FieldValue, property: JSONSchema7): string | null {
  if (typeof value !== 'string') {
    return 'Must be a string'
  }

  if (property.minLength !== undefined && value.length < property.minLength) {
    return `Must be at least ${property.minLength} characters`
  }

  if (property.maxLength !== undefined && value.length > property.maxLength) {
    return `Must be at most ${property.maxLength} characters`
  }

  if (property.format) {
    const formatError = validateFormat(value, property.format)
    if (formatError) {
      return formatError
    }
  }

  if (property.pattern) {
    try {
      const regex = new RegExp(property.pattern)
      if (!regex.test(value)) {
        return 'Invalid format'
      }
    } catch {
      return 'Invalid pattern in schema'
    }
  }

  return null
}

function validateNumber(
  value: FieldValue,
  property: JSONSchema7,
  type: 'number' | 'integer'
): string | null {
  let numValue: number
  if (typeof value === 'string') {
    numValue = parseFloat(value)
  } else if (typeof value === 'number') {
    numValue = value
  } else {
    return 'Must be a number'
  }

  if (isNaN(numValue)) {
    return 'Must be a valid number'
  }

  if (type === 'integer' && !Number.isInteger(numValue)) {
    return 'Must be an integer'
  }

  if (property.minimum !== undefined && numValue < property.minimum) {
    return `Must be at least ${property.minimum}`
  }

  if (property.maximum !== undefined && numValue > property.maximum) {
    return `Must be at most ${property.maximum}`
  }

  return null
}

function validateBoolean(value: FieldValue): string | null {
  if (typeof value !== 'boolean') {
    return 'Must be true or false'
  }
  return null
}

function validateObject(value: FieldValue, property: JSONSchema7, fieldName: string, _schema: JSONSchema7, errors: Record<string, string>): void {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    errors[fieldName] = 'Must be an object'
    return
  }

  if (property.properties) {
    for (const [nestedName, nestedProp] of Object.entries(property.properties)) {
      if (typeof nestedProp === 'boolean') continue

      const nestedValue = (value as Record<string, FieldValue>)[nestedName]
      const nestedPath = `${fieldName}.${nestedName}`

      const nestedRequired = Array.isArray(property.required) && property.required.includes(nestedName)

      if (!nestedRequired && (nestedValue === undefined || nestedValue === null || nestedValue === '')) {
        continue
      }

      if (nestedRequired && (nestedValue === undefined || nestedValue === null || nestedValue === '')) {
        errors[nestedPath] = 'This field is required'
        continue
      }

      const nestedError = validateField(nestedPath, nestedValue, nestedProp)
      if (nestedError) {
        errors[nestedPath] = nestedError
      }
    }
  }
}

function validateArray(value: FieldValue, property?: JSONSchema7): string | null {
  if (!Array.isArray(value)) {
    return 'Must be an array'
  }

  if (!property) return null

  if (property.minItems !== undefined && value.length < property.minItems) {
    return `Must have at least ${property.minItems} items`
  }

  if (property.maxItems !== undefined && value.length > property.maxItems) {
    return `Must have at most ${property.maxItems} items`
  }

  if (property.items && typeof property.items === 'object' && !Array.isArray(property.items)) {
    const itemSchema = property.items as JSONSchema7

    if (itemSchema.enum && Array.isArray(itemSchema.enum)) {
      for (const item of value) {
        const matchesEnum = itemSchema.enum.some(enumVal =>
          enumVal === item || String(enumVal) === String(item)
        )
        if (!matchesEnum) {
          return `All items must be one of: ${itemSchema.enum.join(', ')}`
        }
      }
    }
  }

  return null
}

export function coerceValues(values: FormValues, schema: JSONSchema7): FormValues {
  const coerced: FormValues = { ...values }

  if (!schema.properties) return coerced

  for (const [fieldName, property] of Object.entries(schema.properties)) {
    if (typeof property === 'boolean') continue

    const value = values[fieldName]

    if (value === undefined || value === null || value === '') {
      continue
    }

    const fieldType = getFieldType(property)

    switch (fieldType) {
      case 'number':
      case 'integer':
        if (typeof value === 'string') {
          const numValue = parseFloat(value)
          coerced[fieldName] = isNaN(numValue) ? value : numValue
        } else {
          coerced[fieldName] = value
        }
        break

      case 'boolean':
        if (typeof value === 'string') {
          coerced[fieldName] = value === 'true' || value === '1'
        } else {
          coerced[fieldName] = value
        }
        break

      default:
        coerced[fieldName] = value
    }
  }

  return coerced
}

export function extractSchemaFields(
  values: FormValues,
  schema: JSONSchema7
): FormValues {
  const result: FormValues = {}

  if (!schema.properties) return result

  for (const fieldName of Object.keys(schema.properties)) {
    if (values[fieldName] !== undefined) {
      result[fieldName] = values[fieldName]
    }
  }

  return result
}
